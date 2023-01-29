use std::time::SystemTime;

use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::Instrument;
use webrtc::{
    media::io::h264_reader::H264Reader,
    rtp::packetizer::Packetizer,
    util::{Marshal, MarshalSize},
};

use crate::get_app;

const RTSP_PROTOCOL: &[u8] = b"RTSP/1.0\r\n";
const HTTP_PROTOCOL: &[u8] = b"HTTP/1.1\r\n";

async fn handle_conn(conn: TcpStream) -> Result<()> {
    let mut conn = tokio::io::BufReader::with_capacity(1024 * 10, conn);

    let rtsp_finder = memchr::memmem::Finder::new(RTSP_PROTOCOL);
    let header_end_finder = memchr::memmem::Finder::new(b"\r\n\r\n");

    let mut run = true;
    // This indicates that the client has setup the stream.
    let mut data_tx: Option<tokio::sync::broadcast::Sender<crate::utils::Sample>> = None;
    // This indicates that the client is playing the stream.
    let mut data_rx: Option<tokio::sync::broadcast::Receiver<crate::utils::Sample>> = None;

    let clock_rate = 90000;
    let sequencer: Box<dyn webrtc::rtp::sequence::Sequencer + Send + Sync> =
        Box::new(webrtc::rtp::sequence::new_random_sequencer());
    let mut packetizer = webrtc::rtp::packetizer::new_packetizer(
        1200,
        96, // Value is handled when writing
        0,  // Value is handled when writing
        Box::<webrtc::rtp::codecs::h264::H264Payloader>::default(),
        sequencer.clone(),
        clock_rate,
    );

    let stream_start = SystemTime::now();

    while run {
        let (conn_readable, sample) = if let Some(data_rx) = data_rx.as_mut() {
            (false, data_rx.recv().await.ok())
        } else {
            conn.get_ref().readable().await?;
                (true, None)
        };

        // let (conn_readable, sample) = match (playing, data_rx.as_mut()) {
        //     (true, Some(data_rx)) => {
        //         let data_fut = data_rx.recv();
        //         // let conn_fut = conn.get_mut().readable();
        //         // futures::pin_mut!(data_fut);
        //         // futures::pin_mut!(conn_fut);

        //         // let s = futures::future::select(data_fut, conn_fut).await;
        //         // match s {
        //         //     futures::future::Either::Left((sample, _)) => {
        //         //         let sample = sample?;
        //         //         (false, Some(sample))
        //         //     }
        //         //     futures::future::Either::Right((readable_res, _)) => {
        //         //         readable_res?;
        //         //         (true, None)
        //         //     }
        //         // }
        //         let sample = data_fut.await.ok();
        //         (false, sample)
        //     }
        //     (true, None) => {
        //         anyhow::bail!("Invalid state: playing but no data_rx");
        //     }
        //     (false, _) => {
        //         conn.get_ref().readable().await?;
        //         (true, None)
        //     }
        // };

        if let Some(sample) = sample {
            let timestamp = sample
                .timestamp
                .duration_since(stream_start)
                .map(|d| (d.as_secs_f64() * clock_rate as f64) as u32)
                .unwrap_or(0);

            let data = &sample.data[..];
            let mut h264 = H264Reader::new(std::io::Cursor::new(data));

            while let Ok(nal) = h264.next_nal() {
                let samples = (sample.duration.as_secs_f64() * clock_rate as f64) as u32;
                let packets = packetizer.packetize(&nal.data.freeze(), samples).await?;

                for mut packet in packets {
                    packet.header.timestamp = timestamp;

                    let len = packet.marshal_size();
                    let len_be = (len as u16).to_be_bytes();
                    let mut buf = vec![0; len + 4];
                    buf[0] = b'$';
                    buf[1] = 0;
                    buf[2] = len_be[0];
                    buf[3] = len_be[1];
                    packet.marshal_to(&mut buf[4..])?;
                    conn.write_all(&buf).await?;
                }
            }
        }

        if conn_readable {
            // This should not block, because we are waiting for readable
            let buf = conn.fill_buf().await?;
            if buf[0] == b'$' {
                // RTP/RTCP
                conn.consume(1);
                let channel = conn.read_u8().await?;
                let len = conn.read_u16().await?;

                if len > 0 {
                    let mut body = vec![0; len as usize];
                    conn.read_exact(&mut body).await?;

                    tracing::info!("Got {} bytes of RTP/RTCP data on channel {}", len, channel);
                }
            } else {
                if header_end_finder.find(buf).is_none() {
                    // No enough data to parse headers
                    continue;
                }

                // Very inefficient, but we don't care
                let mut buf = buf.to_vec();
                // Replace with HTTP to make httparse happy
                if let Some(i) = rtsp_finder.find(&buf) {
                    buf[i..i + RTSP_PROTOCOL.len()].copy_from_slice(HTTP_PROTOCOL);
                }

                let mut headers = [httparse::EMPTY_HEADER; 32];
                let mut req = httparse::Request::new(&mut headers);

                if let httparse::Status::Complete(body_offset) = req.parse(&buf)? {
                    // Consume the headers
                    conn.consume(body_offset);

                    let method = req
                        .method
                        .ok_or_else(|| anyhow::anyhow!("Request has no method"))?;

                    let cseq = req
                        .headers
                        .iter()
                        .find(|h| h.name.eq_ignore_ascii_case("cseq"))
                        .ok_or_else(|| anyhow::anyhow!("Request has no CSeq header"))?
                        .value;
                    let cseq: u64 = std::str::from_utf8(cseq)?.parse()?;

                    let content_length: Option<u32> = req
                        .headers
                        .iter()
                        .find(|h| h.name.eq_ignore_ascii_case("content-length"))
                        .map(|h| h.value)
                        .and_then(|v| std::str::from_utf8(v).ok())
                        .and_then(|v| v.parse().ok());

                    // Consume the body
                    if let Some(content_length) = content_length {
                        if content_length > 0 {
                            if content_length > 1024 * 1024 {
                                anyhow::bail!("Content-Length too large");
                            }

                            let mut body = vec![0; content_length as usize];
                            conn.read_exact(&mut body).await?;
                        }
                    }

                    let mut response_lines =
                        vec!["RTSP/1.0 200 OK".to_string(), format!("CSeq: {}", cseq)];
                    let mut response_body = vec![];

                    match method {
                        "OPTIONS" => {
                            tracing::debug!("=> OPTIONS");

                            response_lines.push(
                                "Public: OPTIONS, DESCRIBE, SETUP, TEARDOWN, PLAY, PAUSE".into(),
                            );
                        }
                        "DESCRIBE" => {
                            tracing::debug!("=> DESCRIBE");

                            response_lines.push("Content-Type: application/sdp".into());
                            response_body = concat!(
                                "v=0\n",
                                "o=- 0 0 IN IP4 127.0.0.1\n",
                                "s=No Name\n",
                                "c=IN IP4 127.0.0.1\n",
                                "t=0 0\n",
                                "m=video 0 RTP/AVP/TCP 96\n",
                                "a=rtpmap:96 H264/90000\n",
                            )
                            .as_bytes()
                            .to_vec();
                        }
                        "SETUP" => {
                            tracing::debug!("=> SETUP");

                            let transport = req
                                .headers
                                .iter()
                                .find(|h| h.name.eq_ignore_ascii_case("transport"))
                                .ok_or_else(|| anyhow::anyhow!("Request has no Transport header"))?
                                .value;
                            let transport = std::str::from_utf8(transport)?;
                            response_lines.push(format!("Transport: {}", transport));

                            let data_tx_ = if let Some(monitor) = get_app().monitors().get(&0) {
                                monitor.encoded_tx.clone()
                            } else {
                                anyhow::bail!("Monitor 0 not found");
                            };

                            data_tx = Some(data_tx_);
                        }
                        "TEARDOWN" => {
                            tracing::debug!("=> TEARDOWN");

                            data_tx = None;
                            data_rx = None;
                            run = false;
                        }
                        "PLAY" => {
                            tracing::debug!("=> PLAY");

                            if let Some(data_tx) = data_tx.as_ref() {
                                data_rx = Some(data_tx.subscribe());
                            } else {
                                anyhow::bail!("Invalid state: PLAY without SETUP");
                            }
                        }
                        "PAUSE" => {
                            tracing::debug!("=> PAUSE");

                            data_rx = None;
                        }
                        _ => {}
                    }

                    if !response_body.is_empty() {
                        response_lines.push(format!("Content-Length: {}", response_body.len()));
                    }

                    conn.write_all(response_lines.join("\r\n").as_bytes())
                        .await?;
                    conn.write_all(b"\r\n\r\n").await?;
                    if !response_body.is_empty() {
                        conn.write_all(&response_body).await?;
                    }
                    conn.flush().await?;
                }
            }
        }
    }

    Ok(())
}

async fn rtsp_server() -> Result<()> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9856").await?;

    loop {
        let (socket, addr) = listener.accept().await?;

        let span = tracing::info_span!("rtsp", %addr);
        {
            let _enter = span.enter();
            tracing::info!("New connection");
        }

        tokio::spawn(
            async move {
                if let Err(e) = handle_conn(socket).await {
                    tracing::info!(?e, "Connection terminated");
                }
            }
            .instrument(span),
        );
    }
}

pub fn start() {
    tokio::spawn(async {
        if let Err(e) = rtsp_server().await {
            tracing::error!(?e, "TCP server failed",);
        }
    });
}
