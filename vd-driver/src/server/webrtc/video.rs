use std::sync::Arc;

use tokio::sync::broadcast;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc_media::io::h264_reader::H264Reader;

use crate::utils::Sample;

pub async fn video_sender(
    track: Arc<TrackLocalStaticSample>,
    mut video_data_rx: broadcast::Receiver<Sample>,
) {
    loop {
        match video_data_rx.recv().await {
            Ok(sample) => {
                sample.record_end_to_end_latency();

                let data = &sample.data[..];
                let mut h264 = H264Reader::new(std::io::Cursor::new(data));

                while let Ok(nal) = h264.next_nal() {
                    let res = track
                        .write_sample(&webrtc::media::Sample {
                            data: nal.data.freeze(),
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
