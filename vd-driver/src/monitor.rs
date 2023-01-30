use std::{
    sync::{atomic::AtomicU32, Arc, Mutex},
    time::{Duration, SystemTime},
};

use anyhow::Result;

use crossbeam::channel;
use tokio::sync::broadcast;

use crate::{get_app, utils::Sample};

#[derive(Debug)]
enum EncodingCommand {
    NewFrame(SystemTime),
    Configure {
        width: u32,
        height: u32,
        framerate: u32,
    },
}

#[derive(Debug, Clone)]
pub struct MonitorHandle {
    pub encoded_tx: broadcast::Sender<Sample>,
    width: Arc<AtomicU32>,
    height: Arc<AtomicU32>,
    framerate: Arc<AtomicU32>,
}

impl MonitorHandle {
    pub fn width(&self) -> u32 {
        self.width.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn height(&self) -> u32 {
        self.height.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn framerate(&self) -> u32 {
        self.framerate.load(std::sync::atomic::Ordering::Relaxed)
    }
}

pub struct Monitor {
    cmd_tx: channel::Sender<EncodingCommand>,
    bgra_buffer: Arc<Mutex<Vec<u8>>>,
    /// Connector index of this monitor.
    index: u32,

    width: Arc<AtomicU32>,
    height: Arc<AtomicU32>,
    framerate: Arc<AtomicU32>,
}

impl Monitor {
    pub fn new(index: u32) -> Self {
        let (cmd_tx, cmd_rx) = channel::bounded(1);
        let (data_tx, _) = broadcast::channel(8);

        let bgra_buffer = Arc::new(Mutex::new(Vec::new()));

        let b = bgra_buffer.clone();
        let t = data_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = encoding_thread(cmd_rx, t, b) {
                tracing::error!(?e, "Encoding thread failed");
            }
        });

        let width = Arc::new(AtomicU32::new(0));
        let height = Arc::new(AtomicU32::new(0));
        let framerate = Arc::new(AtomicU32::new(0));

        get_app().register_monitor(
            index,
            MonitorHandle {
                encoded_tx: data_tx,
                width: width.clone(),
                height: height.clone(),
                framerate: framerate.clone(),
            },
        );

        Self {
            cmd_tx,
            bgra_buffer,

            index,

            width,
            height,
            framerate,
        }
    }

    /// Configure the monitor with the given parameters.
    pub fn configure(&mut self, width: u32, height: u32, framerate: u32) {
        self.cmd_tx
            .send(EncodingCommand::Configure {
                width,
                height,
                framerate,
            })
            .ok();

        self.width
            .store(width, std::sync::atomic::Ordering::Relaxed);
        self.height
            .store(height, std::sync::atomic::Ordering::Relaxed);
        self.framerate
            .store(framerate, std::sync::atomic::Ordering::Relaxed);
    }

    /// Notify the monitor that a new frame is available.
    ///
    /// This function is non-blocking, and will return immediately after the data has been copied.
    /// The event is lost if the encoding task is busy.
    pub fn send_frame(&mut self, bgra_buffer: &[u8], timestamp: SystemTime) {
        let mut monitor_buffer = self.bgra_buffer.lock().unwrap();
        monitor_buffer.clear();
        monitor_buffer.extend_from_slice(bgra_buffer);

        self.cmd_tx
            .try_send(EncodingCommand::NewFrame(timestamp))
            .ok();
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        get_app().unregister_monitor(self.index);
    }
}

fn encoding_thread(
    cmd_rx: channel::Receiver<EncodingCommand>,
    data_tx: broadcast::Sender<Sample>,
    bgra_buffer: Arc<Mutex<Vec<u8>>>,
) -> Result<()> {
    crate::utils::set_thread_characteristics();

    let mut session: Option<mfx_dispatch::Session> = Some(mfx_dispatch::Session::new()?);
    let mut encoder: Option<mfx_dispatch::Pipeline> = None;

    let mut width = 0;
    let mut height = 0;
    let mut framerate = 0;
    let mut sample_duration = Duration::from_secs_f64(0.0);

    let mut last_receiver_count = 0;

    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            EncodingCommand::NewFrame(timestamp) => {
                tracing::trace!("New frame");

                let encoder = match encoder.as_mut() {
                    Some(encoder) => encoder,
                    None => continue,
                };

                let receiver_count = data_tx.receiver_count();
                if receiver_count == 0 {
                    if last_receiver_count > 0 {
                        tracing::info!("No more connected clients, stopping encoding");
                        last_receiver_count = 0;
                    }
                    continue;
                } else if receiver_count > last_receiver_count {
                    if last_receiver_count == 0 {
                        tracing::info!("New client connected, starting encoding");
                    }
                    tracing::info!("New client connected, forcing keyframe");
                }

                let dst_stride = encoder.stride();
                let (buf_index, buf_y, buf_uv) = encoder.get_free_surface().unwrap();
                let src = bgra_buffer.lock().unwrap();

                if src.len() != (width * height * 4) as usize {
                    tracing::warn!("Invalid buffer size");
                    continue;
                }

                crate::utils::bgra2nv12(width, height, &src, Some(dst_stride), buf_y, buf_uv)?;

                if let Some(data) =
                    encoder.encode_frame(buf_index, receiver_count > last_receiver_count)?
                {
                    tracing::trace!("Sending frame");

                    let sample = Sample::new(data, timestamp, sample_duration);
                    data_tx.send(sample).ok();
                }

                last_receiver_count = receiver_count;
            }
            EncodingCommand::Configure {
                width: width_,
                height: height_,
                framerate: framerate_,
            } => {
                if width == width_ && height == height_ && framerate == framerate_ {
                    // No change
                    continue;
                }

                width = width_;
                height = height_;
                framerate = framerate_;
                if framerate == 0 {
                    tracing::warn!("Invalid framerate, defaulting to 1");
                    framerate = 1;
                }
                sample_duration = Duration::from_secs_f64(1.0 / framerate as f64);

                let session = match (encoder.take(), session.take()) {
                    (None, Some(session)) => session,
                    (Some(encoder), None) => encoder.close(),
                    (Some(_), Some(_)) => unreachable!(),
                    (None, None) => unreachable!(),
                };

                tracing::info!(?width, ?height, ?framerate, "Configuring encoder with");

                encoder = Some(mfx_dispatch::Pipeline::new(
                    session,
                    width as u16,
                    height as u16,
                    framerate as u16,
                )?);

                tracing::info!("Encoder configured");
            }
        }
    }

    Ok(())
}
