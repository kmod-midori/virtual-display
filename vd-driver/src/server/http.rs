use anyhow::Result;
use axum::{
    extract::Path,
    http::StatusCode,
    routing::{get, post},
    Json,
};
use tokio::sync::mpsc;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::get_app;

use super::webrtc::SdpRequest;

async fn http_server(sdp_tx: mpsc::Sender<SdpRequest>) -> Result<()> {
    let cors = tower_http::cors::CorsLayer::new()
        .allow_methods(tower_http::cors::Any)
        .allow_origin(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../../../web-client/index.html"))
            }),
        )
        .route(
            "/monitors",
            get(|| async {
                let monitors = get_app().monitors().keys().cloned().collect::<Vec<_>>();
                (StatusCode::OK, Json(monitors))
            }),
        )
        .route(
            "/monitors/:id/sdp",
            post(
                |Path(monitor_id): Path<u32>, Json(body): Json<RTCSessionDescription>| async move {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let req = SdpRequest {
                        index: monitor_id,
                        sdp: body,
                        reply: tx,
                    };

                    if sdp_tx.send(req).await.is_ok() {
                        if let Ok(sdp) = rx.await {
                            return (StatusCode::OK, Json(Some(sdp)));
                        }
                    }

                    (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
                },
            ),
        )
        .layer(cors);

    axum::Server::bind(&"0.0.0.0:9000".parse().unwrap())
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

pub fn start(sdp_tx: mpsc::Sender<SdpRequest>) {
    tokio::spawn(async move {
        if let Err(e) = http_server(sdp_tx).await {
            tracing::error!(?e, "HTTP server failed");
        }
    });
}
