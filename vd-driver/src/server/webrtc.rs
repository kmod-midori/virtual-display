use std::sync::Arc;

use anyhow::Result;
use tokio::sync::{broadcast, mpsc, oneshot};
use webrtc::{
    api::media_engine::MediaEngine,
    interceptor::registry::Registry,
    media::io::h264_reader::H264Reader,
    peer_connection::sdp::session_description::RTCSessionDescription,
    track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal},
};

pub struct SdpRequest {
    pub index: u32,
    pub sdp: RTCSessionDescription,
    pub reply: oneshot::Sender<RTCSessionDescription>,
}

async fn webrtc_task(index: u32, sdp: RTCSessionDescription) -> Result<RTCSessionDescription> {
    let mut data_rx = if let Some(m) = crate::get_app().monitors().get(&index) {
        m.encoded_tx.subscribe()
    } else {
        return Err(anyhow::anyhow!("Monitor with index {} not found", index));
    };

    let audio_data_rx = crate::get_app()
        .audio_data_tx
        .as_ref()
        .map(|tx| tx.subscribe());

    let video_track = Arc::new(TrackLocalStaticSample::new(
        webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
            mime_type: webrtc::api::media_engine::MIME_TYPE_H264.to_owned(),
            ..Default::default()
        },
        "video".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    // Feed the video track with data from the encoding task.
    let vt = video_track.clone();
    tokio::spawn(async move {
        loop {
            match data_rx.recv().await {
                Ok(sample) => {
                    sample.record_end_to_end_latency();
                    
                    let data = &sample.data[..];
                    let mut h264 = H264Reader::new(std::io::Cursor::new(data));

                    while let Ok(nal) = h264.next_nal() {
                        let res = vt
                            .write_sample(&webrtc::media::Sample {
                                data: nal.data.freeze(),
                                timestamp: sample.timestamp,
                                duration: sample.duration,
                                ..Default::default()
                            })
                            .await;

                        if let Err(e) = res {
                            tracing::warn!(?e, "Failed to write video sample");
                            break;
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Ignore lagged frames
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    let audio_track = if let Some(mut audio_data_rx) = audio_data_rx {
        let track = Arc::new(TrackLocalStaticSample::new(
            webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
                mime_type: webrtc::api::media_engine::MIME_TYPE_OPUS.to_owned(),
                ..Default::default()
            },
            "audio".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        // Feed the audio track with data from the encoding task.
        let at = track.clone();
        tokio::spawn(async move {
            loop {
                match audio_data_rx.recv().await {
                    Ok(sample) => {
                        let data = sample.data.to_vec();

                        let res = at
                            .write_sample(&webrtc::media::Sample {
                                data: data.into(),
                                timestamp: sample.timestamp,
                                duration: sample.duration,
                                ..Default::default()
                            })
                            .await;

                        if let Err(e) = res {
                            tracing::warn!(?e, "Failed to write video sample");
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Ignore lagged frames
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        Some(track)
    } else {
        None
    };

    let api = {
        let mut m = MediaEngine::default();
        m.register_default_codecs()?;

        // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
        // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
        // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
        // for each PeerConnection.
        let mut registry = Registry::new();

        // Use the default set of Interceptors
        registry =
            webrtc::api::interceptor_registry::register_default_interceptors(registry, &mut m)?;

        webrtc::api::APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build()
    };

    let rtc_config = Default::default();
    let peer_connection = Arc::new(api.new_peer_connection(rtc_config).await?);

    {
        let rtp_sender = peer_connection
            .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        // Read incoming RTCP packets
        // Before these packets are returned they are processed by interceptors. For things
        // like NACK this needs to be called.
        tokio::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
            Result::<()>::Ok(())
        });
    }

    if let Some(audio_track) = &audio_track {
        let rtp_sender = peer_connection
            .add_track(Arc::clone(audio_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        // Read incoming RTCP packets
        // Before these packets are returned they are processed by interceptors. For things
        // like NACK this needs to be called.
        tokio::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
            Result::<()>::Ok(())
        });
    }

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    // let tx = ctx.encoding_cmd_tx.clone();
    peer_connection.on_peer_connection_state_change(Box::new(move |s| {
        tracing::info!("Peer Connection State has changed: {}", s);

        // if s == RTCPeerConnectionState::Connected {
        //     tx.send(EncodingCommand::NewClient).ok();
        // }

        Box::pin(async {})
    }));

    // Set the remote SessionDescription
    peer_connection.set_remote_description(sdp).await?;

    // Create an answer
    let answer = peer_connection.create_answer(None).await?;

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    peer_connection.set_local_description(answer).await?;

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    let _ = gather_complete.recv().await;

    if let Some(local_desc) = peer_connection.local_description().await {
        Ok(local_desc)
    } else {
        Err(anyhow::anyhow!("Failed to get local description"))
    }
}

async fn webrtc_server(mut sdp_rx: mpsc::Receiver<SdpRequest>) {
    while let Some(req) = sdp_rx.recv().await {
        match webrtc_task(req.index, req.sdp).await {
            Ok(sdp) => {
                req.reply.send(sdp).ok();
            }
            Err(e) => {
                tracing::error!(?e, "Failed to get SDP");
            }
        }
    }
}

pub fn start(sdp_rx: mpsc::Receiver<SdpRequest>) {
    tokio::spawn(async move {
        webrtc_server(sdp_rx).await;
        tracing::warn!("WebRTC server task exited");
    });
}
