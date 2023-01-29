pub mod http;
pub mod webrtc;
pub mod tcp;
pub mod rtsp;

pub fn start() {
    let (sdp_tx, sdp_rx) = tokio::sync::mpsc::channel(8);
    http::start(sdp_tx);
    webrtc::start(sdp_rx);
    tcp::start();
    rtsp::start();
}