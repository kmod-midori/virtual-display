use anyhow::{Context, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufStream},
    net::TcpStream,
};

struct AdbConnection {
    stream: BufStream<TcpStream>,
}

impl AdbConnection {
    async fn connect() -> std::io::Result<Self> {
        let stream = TcpStream::connect("127.0.0.1:5037").await?;
        Ok(Self {
            stream: BufStream::new(stream),
        })
    }

    // async fn consume_okay(&mut self) -> Result<()> {
    //     let mut buf = [0u8; 4];
    //     self.stream.read_exact(&mut buf).await?;

    //     if &buf == b"OKAY" {
    //         Ok(())
    //     } else {
    //         Err(anyhow::anyhow!("Expected OKAY"))
    //     }
    // }

    async fn read_result(&mut self) -> Result<()> {
        let mut buf = [0u8; 4];
        self.stream.read_exact(&mut buf).await?;

        match &buf {
            b"OKAY" => Ok(()),
            b"FAIL" => {
                let reason = self.read_packet().await?;
                let reason = String::from_utf8_lossy(&reason);
                Err(anyhow::anyhow!("ADB server returned FAIL: {}", reason))
            }
            _ => Err(anyhow::anyhow!("Expected OKAY or FAIL")),
        }
    }

    async fn read_packet(&mut self) -> Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len_str = std::str::from_utf8(&len_buf).context("Convert packet length to string")?;
        let len = u32::from_str_radix(len_str, 16).context("Parse packet length from hex")?;

        let mut buf = vec![0u8; len as usize];
        self.stream.read_exact(&mut buf).await?;

        Ok(buf)
    }

    async fn write_packet(&mut self, data: &[u8]) -> Result<()> {
        let len = format!("{:04X}", data.len());
        self.stream.write_all(len.as_bytes()).await?;
        self.stream.write_all(data).await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn switch_transport(&mut self, serial: &str) -> Result<()> {
        let pkt = format!("host:transport:{}", serial);
        self.write_packet(pkt.as_bytes()).await?;
        self.read_result().await?;

        Ok(())
    }
}

pub struct AdbClient {}

impl AdbClient {
    async fn enable_reverse(serial: &str, port: u16) -> Result<()> {
        let mut conn = AdbConnection::connect().await?;

        conn.switch_transport(serial).await?;

        let pkt = format!("reverse:forward:tcp:{};tcp:{}", port, port);
        conn.write_packet(pkt.as_bytes()).await?;

        conn.read_result().await.context("Send command")?;
        conn.read_result().await.context("Enable reverse")?;

        Ok(())
    }

    async fn track_devices() -> Result<()> {
        let mut conn = AdbConnection::connect().await?;

        conn.write_packet(b"host:track-devices").await?;
        conn.read_result()
            .await
            .context("Start tracking connected devices")?;

        tracing::info!("Started tracking devices");

        loop {
            let packet = conn.read_packet().await?;
            let devices = Self::parse_devices(std::str::from_utf8(&packet)?)
                .context("Parse connected devices")?;
            for (serial, state) in devices {
                if state != "device" {
                    continue;
                }

                tracing::info!(?serial, "Found device");

                let serial = serial.to_owned();
                tokio::spawn(async move {
                    if let Err(e) = Self::enable_reverse(&serial, 9867).await {
                        tracing::error!(?e, ?serial, "Failed to enable reverse");
                    } else {
                        tracing::info!(?serial, "Enabled reverse");
                    }
                });
            }
        }
    }

    fn parse_devices(packet: &str) -> Result<Vec<(&str, &str)>> {
        let packet = packet.trim();

        let mut devices = Vec::new();

        for line in packet.lines() {
            let mut parts = line.split_whitespace();
            let serial = parts.next().context("Device serial")?;
            let state = parts.next().context("Device state")?;
            devices.push((serial, state));
        }

        Ok(devices)
    }

    pub fn start() {
        tokio::spawn(Self::run_internal());
    }

    async fn run_internal() {
        loop {
            if let Err(e) = Self::track_devices().await {
                tracing::error!(?e, "Failed to track devices");
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    }
}
