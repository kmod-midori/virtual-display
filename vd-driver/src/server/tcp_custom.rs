use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufStream},
    net::TcpStream,
};
use tracing::Instrument;

use crate::{get_app, monitor::CodecData};

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PacketType {
    /// `[i64 ts][data]`
    Video = 0,
    /// `[i64 ts][data]`
    Audio = 1,
    /// `[i64 ts]`
    Timestamp = 2,
    /// `[i32 width][i32 height][u32 len][data][u32 len][data]...`
    Configure = 3,
    /// `[i32 x][i32 y][u32 visible]`
    CursorPosition = 4,
    /// `[u32 crc32][data]`
    CursorImage = 5,
}

#[derive(Debug)]
struct VdStream {
    inner: BufStream<TcpStream>,
}

impl VdStream {
    async fn write_packet(&mut self, ty: PacketType, data: &[&[u8]]) -> Result<()> {
        let total_len = data.iter().map(|d| d.len()).sum::<usize>();

        tracing::trace!("Writing {:?} ({} bytes)", ty, total_len);

        let task = async move {
            self.inner.write_u32(ty as u32).await?;
            self.inner.write_u32(total_len as u32).await?;
            for d in data {
                self.inner.write_all(d).await?;
            }

            std::io::Result::Ok(())
        };

        tokio::time::timeout(TIMEOUT, task).await??;

        Ok(())
    }

    async fn write_timestamp(&mut self, timestamp: u64) -> Result<()> {
        self.write_packet(PacketType::Timestamp, &[&(timestamp as i64).to_be_bytes()])
            .await
    }

    async fn write_video(&mut self, timestamp: u64, data: &[u8]) -> Result<()> {
        self.write_packet(
            PacketType::Video,
            &[&(timestamp as i64).to_be_bytes(), data],
        )
        .await
    }

    async fn write_configure(&mut self, width: u32, height: u32, data: &CodecData) -> Result<()> {
        let mut pkts = vec![
            (width as i32).to_be_bytes().to_vec(),
            (height as i32).to_be_bytes().to_vec(),
        ];

        match data {
            CodecData::H264 { sps, pps } => {
                pkts.push((sps.len() as u32).to_be_bytes().to_vec());
                pkts.push(sps.to_vec());
                pkts.push((pps.len() as u32).to_be_bytes().to_vec());
                pkts.push(pps.to_vec());
            }
        }

        let pkts_ref: Vec<&[u8]> = pkts.iter().map(|v| v.as_slice()).collect();
        self.write_packet(PacketType::Configure, &pkts_ref).await
    }

    async fn write_cursor_position(&mut self, x: i32, y: i32, visible: bool) -> Result<()> {
        self.write_packet(
            PacketType::CursorPosition,
            &[
                &x.to_be_bytes(),
                &y.to_be_bytes(),
                &(visible as u32).to_be_bytes(),
            ],
        )
        .await
    }

    async fn write_cursor_image(&mut self, crc32: u32, data: &[u8]) -> Result<()> {
        self.write_packet(PacketType::CursorImage, &[&crc32.to_be_bytes(), data])
            .await
    }

    async fn flush(&mut self) -> Result<()> {
        tokio::time::timeout(TIMEOUT, self.inner.flush()).await??;
        Ok(())
    }
}

async fn handle(socket: TcpStream) -> Result<()> {
    socket.set_nodelay(true).ok();

    let socket = tokio::io::BufStream::with_capacity(1024, 1024 * 1024 * 8, socket);
    let mut stream = VdStream { inner: socket };

    let monitor_id = stream.inner.read_u32().await?;
    let monitor = if let Some(monitor) = get_app().monitors().get(&monitor_id) {
        monitor.clone()
    } else {
        anyhow::bail!("Monitor {} not found", monitor_id);
    };

    let mut video_data_rx = monitor.encoded_tx.subscribe();
    let mut codec_data_rx = monitor.codec_data();
    let mut cursor_position_rx = monitor.cursor_position();
    let mut cursor_image_rx = monitor.cursor_image();

    // == Timing

    let stream_start = Instant::now();

    {
        // Send initial timestamp packet
        let now = stream_start.elapsed().as_millis() as u64;
        stream.write_timestamp(now).await?;
        stream.flush().await?;
    }

    let mut timestamp_interval = tokio::time::interval(Duration::from_secs(10));

    // == Codec configuration

    let encoder_data = loop {
        tracing::info!("Waiting for codec data");

        if codec_data_rx.changed().await.is_err() {
            anyhow::bail!("Encoder data channel closed");
        }

        let r = codec_data_rx.borrow();
        if let Some(data) = r.as_ref() {
            break data.clone();
        }
    };

    tracing::info!("Obtained codec data");

    stream
        .write_configure(monitor.width(), monitor.height(), &encoder_data)
        .await?;

    // == Frames

    loop {
        tokio::select! {
            _ = cursor_position_rx.changed() => {
                let cursor_pos = {
                    let cursor_pos_ref = cursor_position_rx.borrow();
                    if let Some(p) = cursor_pos_ref.as_ref() {
                        *p
                    } else {
                        continue;
                    }
                };

                stream.write_cursor_position(cursor_pos.x, cursor_pos.y, cursor_pos.visible).await?;
            }
            _ = cursor_image_rx.changed() => {
                let cursor_image = {
                    let cursor_image_ref = cursor_image_rx.borrow();
                    if let Some(p) = cursor_image_ref.as_ref() {
                        p.clone()
                    } else {
                        continue;
                    }
                };

                stream.write_cursor_image(cursor_image.crc32, &cursor_image.encoded).await?;
            }
            _ = codec_data_rx.changed() => {
                let codec_data = {
                    let codec_data_ref = codec_data_rx.borrow();
                    if let Some(p) = codec_data_ref.as_ref() {
                        p.clone()
                    } else {
                        continue;
                    }
                };

                stream
                    .write_configure(monitor.width(), monitor.height(), &codec_data)
                    .await?;
            }
            _ = timestamp_interval.tick() => {
                let now = stream_start.elapsed().as_millis() as u64;
                stream.write_timestamp(now).await?;
            }
            sample = video_data_rx.recv() => {
                let sample = if let Ok(sample) = sample {
                    sample
                } else {
                    break;
                };

                sample.record_end_to_end_latency();

                stream
                    .write_video(
                        sample.timestamp.duration_since(stream_start).as_millis() as u64,
                        &sample.data,
                    )
                    .await?;
            }
        }
        stream.flush().await?;
    }

    Ok(())
}

async fn tcp_server() -> Result<()> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9867").await?;

    loop {
        let (socket, addr) = listener.accept().await?;

        let span = tracing::info_span!("tcp_custom", %addr);
        {
            let _enter = span.enter();
            tracing::info!("New connection");
        }

        tokio::spawn(
            async move {
                if let Err(e) = handle(socket).await {
                    tracing::error!(?e, "Connection failed");
                }
            }
            .instrument(span),
        );
    }
}

pub fn start() {
    tokio::spawn(async {
        if let Err(e) = tcp_server().await {
            tracing::error!(?e, "TCP server failed",);
        }
    });
}
