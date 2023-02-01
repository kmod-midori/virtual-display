use std::time::Instant;

use anyhow::Result;
use tokio::{
    io::{AsyncWrite, AsyncWriteExt, BufWriter},
    net::{TcpSocket, TcpStream},
};
use tracing::Instrument;

use crate::{get_app, monitor::CodecData};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PacketType {
    /// `[i64 ts][data]`
    Video = 0,
    Audio = 1,
    /// `[i64 ts]`
    Timestamp = 2,
    /// `[u32 len][data][u32 len][data]...`
    CodecData = 3,
}

#[derive(Debug)]
struct VdStream {
    inner: BufWriter<TcpStream>,
}

impl VdStream {
    async fn write_packet(&mut self, ty: PacketType, data: &[&[u8]]) -> Result<()> {
        let total_len = data.iter().map(|d| d.len()).sum::<usize>();

        tracing::trace!("Writing {:?} ({} bytes)", ty, total_len);

        self.inner.write_u32(ty as u32).await?;
        self.inner.write_u32(total_len as u32).await?;
        for d in data {
            self.inner.write_all(d).await?;
        }

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

    async fn write_codec_data(&mut self, data: &[&[u8]]) -> Result<()> {
        let mut pkts = vec![];

        for d in data {
            pkts.push((d.len() as u32).to_be_bytes().to_vec());
            pkts.push(d.to_vec());
        }

        self.write_packet(PacketType::CodecData, data).await
    }

    async fn flush(&mut self) -> Result<()> {
        self.inner.flush().await?;
        Ok(())
    }
}

async fn handle(socket: TcpStream) -> Result<()> {
    socket.set_nodelay(true).ok();

    let socket = tokio::io::BufWriter::with_capacity(1024 * 1024 * 8, socket);

    let monitor = if let Some(monitor) = get_app().monitors().get(&0) {
        monitor.clone()
    } else {
        anyhow::bail!("Monitor 0 not found");
    };
    let mut data_rx = monitor.encoded_tx.subscribe();
    let mut encoder_data_rx = monitor.codec_data();

    let mut stream = VdStream { inner: socket };

    // == Timing

    let stream_start = Instant::now();

    {
        // Send initial timestamp packet
        let now = stream_start.elapsed().as_millis() as u64;
        stream.write_timestamp(now).await?;
        stream.flush().await?;
    }

    // Last time we sent a timestamp packet
    let mut last_timestamp_written = Instant::now();

    // == Codec configuration

    let encoder_data = loop {
        tracing::info!("Waiting for codec data");

        if encoder_data_rx.changed().await.is_err() {
            anyhow::bail!("Encoder data channel closed");
        }

        let r = encoder_data_rx.borrow();
        if let Some(data) = r.as_ref() {
            break data.clone();
        }
    };

    tracing::info!("Obtained codec data");

    match encoder_data {
        CodecData::H264 { sps, pps } => {
            stream.write_codec_data(&[&sps[..], &pps[..]]).await?;
        }
    }

    // == Frames

    while let Ok(sample) = data_rx.recv().await {
        sample.record_end_to_end_latency();

        stream
            .write_video(
                sample.timestamp.duration_since(stream_start).as_millis() as u64,
                &sample.data,
            )
            .await?;

        if last_timestamp_written.elapsed() > std::time::Duration::from_secs(10) {
            // Send a timestamp packet every 10 seconds, to sync the stream
            let now = stream_start.elapsed().as_millis() as u64;
            stream.write_timestamp(now).await?;

            last_timestamp_written = Instant::now();
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
