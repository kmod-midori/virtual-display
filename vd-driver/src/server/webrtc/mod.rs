use std::sync::Arc;

use anyhow::Result;
use bytes::{BufMut, BytesMut};
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;
use webrtc::{
    api::media_engine::MediaEngine,
    data_channel::data_channel_init::RTCDataChannelInit,
    interceptor::registry::Registry,
    peer_connection::{
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal},
};

mod audio;
mod video;

pub struct SdpRequest {
    pub index: u32,
    pub sdp: RTCSessionDescription,
    pub reply: oneshot::Sender<RTCSessionDescription>,
}

async fn webrtc_task(index: u32, sdp: RTCSessionDescription) -> Result<RTCSessionDescription> {
    let monitor = if let Some(m) = crate::get_app().monitors().get(&index) {
        m.clone()
    } else {
        return Err(anyhow::anyhow!("Monitor with index {} not found", index));
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

    let span = tracing::info_span!(
        "webrtc",
        monitor = index,
        conn = peer_connection.get_stats_id()
    );
    let _enter = span.enter();

    let done = Arc::new(tokio::sync::Notify::new());

    let video_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: webrtc::api::media_engine::MIME_TYPE_H264.to_owned(),
            clock_rate: 90000,
            ..Default::default()
        },
        "video".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    // Feed the video track with data from the encoding task.
    let vt = video_track.clone();
    let video_data_rx = monitor.encoded_tx.subscribe();
    let done_ = done.clone();
    tokio::spawn(
        async move {
            tokio::select! {
                _ = video::video_sender(vt, video_data_rx) => {}
                _ = done_.notified() => {}
            }
            tracing::info!("Video track done");
        }
        .instrument(span.clone()),
    );

    let audio_data_rx = crate::get_app()
        .audio_data_tx
        .as_ref()
        .map(|tx| tx.subscribe());
    let audio_track = if let Some(audio_data_rx) = audio_data_rx {
        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: webrtc::api::media_engine::MIME_TYPE_OPUS.to_owned(),
                ..Default::default()
            },
            "audio".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        // Feed the audio track with data from the encoding task.
        let at = track.clone();
        let done_ = done.clone();
        tokio::spawn(
            async move {
                tokio::select! {
                    _ = audio::audio_sender(at, audio_data_rx) => {}
                    _ = done_.notified() => {}
                }
                tracing::info!("Audio track done");
            }
            .instrument(span.clone()),
        );

        Some(track)
    } else {
        None
    };

    // Video
    {
        let rtp_sender = peer_connection
            .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        tokio::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
            Result::<()>::Ok(())
        });
    }

    // Audio
    if let Some(audio_track) = &audio_track {
        let rtp_sender = peer_connection
            .add_track(Arc::clone(audio_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        tokio::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
            Result::<()>::Ok(())
        });
    }

    // Cursor
    let control_data_channel = peer_connection
        .create_data_channel(
            "control",
            Some(RTCDataChannelInit {
                negotiated: Some(0),
                ..Default::default()
            }),
        )
        .await?;

    let mut cursor_position_rx = monitor.cursor_position();
    let mut cursor_image_rx = monitor.cursor_image();
    let done_ = done.clone();
    let span_ = span.clone();
    
    let control_data_channel_ = control_data_channel.clone();
    control_data_channel.on_open(Box::new(move || {
        {
            let _enter = span_.enter();
            tracing::info!("Control data channel opened");
        }

        let ch = control_data_channel_.clone();
        Box::pin(
            async move {
                loop {
                    let done_fut = done_.notified();
                    let cursor_pos_fut = cursor_position_rx.changed();
                    let cursor_image_fut = cursor_image_rx.changed();

                    tokio::select! {
                        _ = done_fut => {
                            break;
                        }
                        _ = cursor_pos_fut => {
                            let cursor_pos = {
                                let cursor_pos_ref = cursor_position_rx.borrow();
                                if let Some(p) = cursor_pos_ref.as_ref() {
                                    *p
                                } else {
                                    continue;
                                }
                            };

                            let mut buffer = BytesMut::with_capacity(10);
                            buffer.put_u8(0);
                            buffer.put_i32(cursor_pos.x);
                            buffer.put_i32(cursor_pos.y);
                            buffer.put_u8(cursor_pos.visible as u8);

                            if let Err(e) = ch.send(&buffer.freeze()).await {
                                tracing::warn!(?e, "Failed to send cursor position");
                                break;
                            }
                        }
                        _ = cursor_image_fut => {
                            let cursor_image = {
                                let cursor_image_ref = cursor_image_rx.borrow();
                                if let Some(p) = cursor_image_ref.as_ref() {
                                    p.clone()
                                } else {
                                    continue;
                                }
                            };

                            let mut buffer = BytesMut::with_capacity(cursor_image.encoded.len() + 5);
                            buffer.put_u8(1);
                            buffer.put_u32(cursor_image.crc32);
                            buffer.put(cursor_image.encoded);

                            if let Err(e) = ch.send(&buffer.freeze()).await {
                                tracing::warn!(?e, "Failed to send cursor image");
                                break;
                            }
                        }
                    }
                }
            }
            .instrument(span_),
        )
    }));

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    // let tx = ctx.encoding_cmd_tx.clone();
    peer_connection.on_peer_connection_state_change(Box::new(move |s| {
        if s == RTCPeerConnectionState::Failed {
            // Connection has failed, close everything
            done.notify_waiters();
        }

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
