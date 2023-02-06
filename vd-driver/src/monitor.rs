use std::{
    num::NonZeroUsize,
    sync::{atomic::AtomicU32, Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;

use bytes::Bytes;
use crossbeam::channel;
use image::{ImageBuffer, ImageOutputFormat, Rgba};
use lru::LruCache;
use mfx_dispatch::builder::{EncoderConfig, RateControlMethod, RequiredFields};
use tokio::sync::{broadcast, watch};

use crate::{get_app, utils::Sample};

#[derive(Debug)]
enum EncodingCommand {
    NewFrame(Instant),
    Configure {
        width: u32,
        height: u32,
        framerate: u32,
    },
}

#[derive(Debug, Clone)]
pub enum VideoCodecData {
    H264 { sps: Bytes, pps: Bytes },
}

impl VideoCodecData {
    pub fn mime(&self) -> &'static str {
        match self {
            VideoCodecData::H264 { .. } => "video/avc",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitorHandle {
    pub encoded_tx: broadcast::Sender<Sample>,
    codec_data_rx: watch::Receiver<Option<VideoCodecData>>,

    width: Arc<AtomicU32>,
    height: Arc<AtomicU32>,
    framerate: Arc<AtomicU32>,

    cursor_position_rx: watch::Receiver<Option<CursorPosition>>,
    cursor_image_rx: watch::Receiver<Option<CursorImage>>,
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

    pub fn codec_data(&self) -> watch::Receiver<Option<VideoCodecData>> {
        self.codec_data_rx.clone()
    }

    pub fn cursor_position(&self) -> watch::Receiver<Option<CursorPosition>> {
        self.cursor_position_rx.clone()
    }

    pub fn cursor_image(&self) -> watch::Receiver<Option<CursorImage>> {
        self.cursor_image_rx.clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CursorPosition {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct CursorImage {
    pub crc32: u32,
    pub raw: ImageBuffer<Rgba<u8>, Bytes>,
    pub encoded: Bytes,
}

impl CursorImage {
    pub fn width(&self) -> u32 {
        self.raw.width()
    }

    pub fn height(&self) -> u32 {
        self.raw.height()
    }
}

pub struct Monitor {
    cmd_tx: channel::Sender<EncodingCommand>,
    bgra_buffer: Arc<Mutex<Vec<u8>>>,

    cursor_cache: Mutex<LruCache<u32, CursorImage>>,

    /// Connector index of this monitor.
    index: u32,

    width: Arc<AtomicU32>,
    height: Arc<AtomicU32>,
    framerate: Arc<AtomicU32>,

    cursor_position_tx: watch::Sender<Option<CursorPosition>>,
    cursor_image_tx: watch::Sender<Option<CursorImage>>,
}

impl Monitor {
    pub fn new(index: u32) -> Self {
        let (cmd_tx, cmd_rx) = channel::bounded(1);
        let (data_tx, _) = broadcast::channel(8);
        let (codec_data_tx, encoder_data_rx) = watch::channel(None);
        let (cursor_position_tx, cursor_position_rx) = watch::channel(None);
        let (cursor_image_tx, cursor_image_rx) = watch::channel(None);
        let bgra_buffer = Arc::new(Mutex::new(Vec::new()));

        let b = bgra_buffer.clone();
        let t = data_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = encoding_thread(cmd_rx, t, codec_data_tx, b) {
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
                codec_data_rx: encoder_data_rx,
                width: width.clone(),
                height: height.clone(),
                framerate: framerate.clone(),

                cursor_position_rx,
                cursor_image_rx,
            },
        );

        Self {
            cmd_tx,
            bgra_buffer,

            cursor_cache: Mutex::new(LruCache::new(NonZeroUsize::new(60).unwrap())),

            index,

            width,
            height,
            framerate,

            cursor_position_tx,
            cursor_image_tx,
        }
    }

    /// Configure the monitor with the given parameters.
    pub fn configure(&self, width: u32, height: u32, framerate: u32) {
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

    pub fn width(&self) -> u32 {
        self.width.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn height(&self) -> u32 {
        self.height.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Notify the monitor that a new frame is available.
    ///
    /// This function is non-blocking, and will return immediately after the data has been copied.
    /// The event is lost if the encoding task is busy.
    pub fn send_frame(&self, bgra_buffer: &[u8], timestamp: Instant) {
        let mut monitor_buffer = self.bgra_buffer.lock().unwrap();
        monitor_buffer.clear();
        monitor_buffer.extend_from_slice(bgra_buffer);

        self.cmd_tx
            .try_send(EncodingCommand::NewFrame(timestamp))
            .ok();
    }

    pub fn set_cursor_position(&self, x: i32, y: i32, visible: bool) {
        let pos = CursorPosition { x, y, visible };
        self.cursor_position_tx.send(Some(pos)).ok();
    }

    pub fn set_cursor_image(&self, width: u32, height: u32, mut image: Vec<u8>) {
        for chunk in image.chunks_exact_mut(4) {
            let b = chunk[0];
            let g = chunk[1];
            let r = chunk[2];
            let a = chunk[3];

            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = a;
        }

        let data = Bytes::from(image);
        let checksum = crc32fast::hash(&data);

        let mut cache = self.cursor_cache.lock().unwrap();
        let cursor_image = if let Some(c) = cache.get(&checksum).cloned() {
            c
        } else {
            let raw = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data).unwrap();

            let mut encoded = std::io::Cursor::new(Vec::new());
            let encoded = match raw.write_to(&mut encoded, ImageOutputFormat::Png) {
                Ok(_) => Bytes::from(encoded.into_inner()),
                Err(e) => {
                    tracing::error!(?e, "Failed to encode cursor image");
                    return;
                }
            };

            let c = CursorImage {
                crc32: checksum,
                raw,
                encoded,
            };
            cache.put(checksum, c.clone());
            c
        };

        self.cursor_image_tx.send(Some(cursor_image)).ok();
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
    codec_data_tx: watch::Sender<Option<VideoCodecData>>,
    bgra_buffer: Arc<Mutex<Vec<u8>>>,
) -> Result<()> {
    crate::utils::set_thread_characteristics();

    let metrics = crate::metrics::get_metrics();
    let encoded_frames_local = metrics.encoded_frames.local();
    let encoding_latency_ms_local = metrics.encoding_latency_ms.local();

    let mut encoder: Option<mfx_dispatch::Pipeline<mfx_dispatch::buffer::BgraBuffer>> = None;

    let mut width = 0u32;
    let mut height = 0u32;
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

                let (buf_index, buf) = encoder.get_free_surface().unwrap();
                let dst_stride = buf.stride();
                let src = bgra_buffer.lock().unwrap();

                if src.len() != (width * height * 4) as usize {
                    tracing::warn!("Invalid buffer size");
                    continue;
                }

                for row in 0..height {
                    let src_offset = (row * width * 4) as usize;
                    let dst_offset = (row * dst_stride as u32) as usize;
                    let len = (width * 4) as usize;

                    buf.data_mut()[dst_offset..dst_offset + len]
                        .copy_from_slice(&src[src_offset..src_offset + len]);
                }

                let encoding_start = Instant::now();
                if let Some(data) =
                    encoder.encode_frame(buf_index, receiver_count > last_receiver_count)?
                {
                    encoded_frames_local.inc();
                    encoding_latency_ms_local
                        .observe(encoding_start.elapsed().as_secs_f64() * 1000.0);

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

                tracing::info!(?width, ?height, ?framerate, "Configuring encoder with");

                let config = EncoderConfig::new(RequiredFields {
                    width: width as u16,
                    height: height as u16,
                    codec: mfx_dispatch::builder::Codec::H264 {
                        profile: Some(mfx_dispatch::builder::H264Profile::Baseline),
                    },
                    framerate: (framerate as u16, 1),
                })
                .with_rate_control(RateControlMethod::IntelligentConstantQuality { quality: 10 });

                let e = if let Some(encoder) = &mut encoder {
                    encoder.reset(config)?;
                    encoder
                } else {
                    encoder = Some(mfx_dispatch::Pipeline::new(config)?);
                    encoder.as_mut().unwrap()
                };

                codec_data_tx
                    .send(Some(VideoCodecData::H264 {
                        sps: e.sps().to_vec().into(),
                        pps: e.pps().to_vec().into(),
                    }))
                    .ok();

                tracing::info!("Encoder configured");
            }
        }

        if encoded_frames_local.get() > 120 {
            // Flush metrics to the global registry every 2 seconds
            encoded_frames_local.flush();
            encoding_latency_ms_local.flush();
        }
    }

    Ok(())
}
