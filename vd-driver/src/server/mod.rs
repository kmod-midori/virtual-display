pub mod http;
pub mod rtsp;
pub mod tcp;
#[cfg(feature = "webrtc")]
pub mod webrtc;

pub fn start() {
    #[cfg(feature = "webrtc")]
    let (sdp_tx, sdp_rx) = tokio::sync::mpsc::channel(8);

    let http_ctx = http::HttpServerContext {
        #[cfg(feature = "webrtc")]
        sdp_tx,
    };
    http::start(http_ctx);

    #[cfg(feature = "webrtc")]
    webrtc::start(sdp_rx);

    tcp::start();

    rtsp::start();
}
