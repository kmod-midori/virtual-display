use std::time::SystemTime;

use anyhow::Result;
use axum::http::StatusCode;
use rtp::{packetizer::Packetizer, sequence::Sequencer};
use sdp::{
    description::media::{MediaName, RangedPort},
    MediaDescription, SessionDescription,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::Instrument;
use webrtc_util::{Marshal, MarshalSize};

use crate::get_app;

const RTSP_PROTOCOL: &[u8] = b"RTSP/1.0\r\n";
const HTTP_PROTOCOL: &[u8] = b"HTTP/1.1\r\n";

fn video_sdp() -> String {
    // m=video 0 RTP/AVP/TCP 96
    let media_desc = MediaDescription {
        media_name: MediaName {
            media: "video".into(),
            port: RangedPort {
                value: 0,
                range: None,
            },
            protos: vec!["RTP".into(), "AVP".into(), "TCP".into()],
            // Taken care of by `with_codec`.
            formats: vec![],
        },
        ..Default::default()
    }
    .with_codec(96, "H264".into(), 90000, 0, Default::default());

    let origin = sdp::description::session::Origin {
        username: "-".into(),
        session_id: 0,
        session_version: 0,
        network_type: "IN".into(),
        address_type: "IP4".into(),
        unicast_address: "0.0.0.0".into(),
    };

    let conn_info = sdp::description::common::ConnectionInformation {
        network_type: "IN".into(),
        address_type: "IP4".into(),
        address: Some(sdp::description::common::Address {
            address: "0.0.0.0".into(),
            ttl: None,
            range: None,
        }),
    };

    let sdp = SessionDescription {
        version: 0,
        origin,
        session_name: "Display 0".into(),
        connection_information: Some(conn_info),
        media_descriptions: vec![media_desc],
        ..Default::default()
    };

    sdp.marshal()
}

fn find_and_decode_header(headers: &[httparse::Header], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .and_then(|h| String::from_utf8(h.value.to_vec()).ok())
}

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
    let sequencer: Box<dyn Sequencer + Send + Sync> =
        Box::new(rtp::sequence::new_random_sequencer());
    let mut packetizer = rtp::packetizer::new_packetizer(
        1200,
        96, // Value is handled when writing
        0,  // Value is handled when writing
        Box::<rtp::codecs::h264::H264Payloader>::default(),
        sequencer.clone(),
        clock_rate,
    );

    let stream_start = SystemTime::now();

    while run {
        let (conn_readable, sample) = match data_rx.as_mut() {
            Some(data_rx) => {
                // We are playing
                // let data_fut = ;
                // let conn_fut = ;

                // tokio::select! {
                //     sample = data_rx.recv() => {
                //         let sample = sample.ok();
                //         (false, sample)
                //     }
                //     _ = conn.get_mut().readable() => {
                //         (true, None)
                //     }
                // }

                // // futures::pin_mut!(data_fut);
                // // futures::pin_mut!(conn_fut);

                // // let s = futures::future::select(data_fut, conn_fut).await;
                // // match s {
                // //     futures::future::Either::Left((sample, _)) => {
                // //         let sample = sample?;
                // //         (false, Some(sample))
                // //     }
                // //     futures::future::Either::Right((readable_res, _)) => {
                // //         readable_res?;
                // //         (true, None)
                // //     }
                // // }

                let sample = data_rx.recv().await.ok();
                (false, sample)
            }
            None => {
                // We are not playing
                conn.get_ref().readable().await?;
                (true, None)
            }
        };

        if let Some(sample) = sample {
            let timestamp = sample
                .timestamp
                .duration_since(stream_start)
                .map(|d| (d.as_secs_f64() * clock_rate as f64) as u32)
                .unwrap_or(0);

            let data = &sample.data[..];
            let mut h264 =
                webrtc_media::io::h264_reader::H264Reader::new(std::io::Cursor::new(data));

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
            if buf.is_empty() {
                // EOF
                run = false;
                continue;
            }

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

                    let req_content_length: Option<u32> = req
                        .headers
                        .iter()
                        .find(|h| h.name.eq_ignore_ascii_case("content-length"))
                        .map(|h| h.value)
                        .and_then(|v| std::str::from_utf8(v).ok())
                        .and_then(|v| v.parse().ok());

                    // Consume the body
                    if let Some(content_length) = req_content_length {
                        if content_length > 0 {
                            if content_length > 1024 * 1024 {
                                anyhow::bail!("Content-Length too large");
                            }

                            let mut body = vec![0; content_length as usize];
                            conn.read_exact(&mut body).await?;
                        }
                    }

                    let mut status_code = StatusCode::OK;

                    let mut response_lines = vec![];
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

                            response_lines.push("Content-Type: application/sdp".to_string());
                            response_body = video_sdp().as_bytes().to_vec();
                        }
                        "SETUP" => {
                            tracing::debug!("=> SETUP");

                            let monitor_id = 0;

                            if let Some(monitor) = get_app().monitors().get(&monitor_id) {
                                // Force TCP mode
                                response_lines.push("Transport: RTP/AVP/TCP;unicast;interleaved=0-1".to_string());

                                data_tx = Some(monitor.encoded_tx.clone());
                            } else {
                                tracing::error!("Monitor {} not found", monitor_id);
                                status_code = StatusCode::NOT_FOUND;
                            };
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
                                tracing::error!("Invalid state: PLAY without SETUP");
                                status_code = StatusCode::BAD_REQUEST;
                            }
                        }
                        "PAUSE" => {
                            tracing::debug!("=> PAUSE");

                            data_rx = None;
                        }
                        _ => {}
                    }

                    conn.write_all(
                        format!(
                            "RTSP/1.0 {} {}\r\nCSeq: {}\r\n",
                            status_code.as_u16(),
                            status_code.canonical_reason().unwrap_or(""), // Should never be None in our use case
                            cseq
                        )
                        .as_bytes(),
                    )
                    .await?;

                    if !response_body.is_empty() {
                        conn.write_all(
                            format!("Content-Length: {}\r\n", response_body.len()).as_bytes(),
                        )
                        .await?;
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
