use std::sync::Arc;

use tokio::sync::broadcast;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;

use crate::utils::Sample;

pub async fn audio_sender(
    track: Arc<TrackLocalStaticSample>,
    mut audio_data_rx: broadcast::Receiver<Sample>,
) {
    loop {
        match audio_data_rx.recv().await {
            Ok(sample) => {
                let data = sample.data.to_vec();

                let res = track
                    .write_sample(&webrtc::media::Sample {
                        data: data.into(),
                        // not really used in the stack
                        // timestamp: sample.timestamp,
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
}
