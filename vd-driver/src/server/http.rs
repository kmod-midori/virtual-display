#![allow(dead_code, unused_imports, unused_variables)]

use anyhow::Result;
use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::Path,
    http::StatusCode,
    routing::{get, post},
    Json,
};
use tokio::sync::mpsc;

use crate::{get_app, monitor::MonitorHandle};

#[derive(Template)]
#[template(path = "index.html")]
struct HomeTemplate<'a> {
    monitors: &'a [(u32, &'a MonitorHandle)],
    rtsp_port: u16,
}

#[derive(Template)]
#[template(path = "webrtc.html")]
struct WebrtcTemplate {
    id: u32,
}

pub(super) struct HttpServerContext {
    #[cfg(feature = "webrtc")]
    pub sdp_tx: mpsc::Sender<super::webrtc::SdpRequest>,
}

async fn http_server(ctx: HttpServerContext) -> Result<()> {
    let cors = tower_http::cors::CorsLayer::new()
        .allow_methods(tower_http::cors::Any)
        .allow_origin(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    let mut app = axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async {
                let monitors = get_app().monitors();
                let m = monitors.iter().map(|(k, v)| (*k, v)).collect::<Vec<_>>();
                HomeTemplate {
                    monitors: &m[..],
                    rtsp_port: 9856,
                }
                .into_response()
            }),
        )
        .route(
            "/metrics",
            get(|| async {
                let c = prometheus::gather();
                prometheus::TextEncoder::new()
                    .encode_to_string(&c)
                    .unwrap_or_default()
            }),
        )
        .route(
            "/monitors",
            get(|| async {
                let monitors = get_app().monitors().keys().cloned().collect::<Vec<_>>();
                (StatusCode::OK, Json(monitors))
            }),
        );

    #[cfg(feature = "webrtc")]
    {
        use super::webrtc::SdpRequest;
        use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

        let sdp_tx = ctx.sdp_tx;

        app = app
            .route(
                "/webrtc/:id",
                get(|Path(monitor_id): Path<u32>| async move {
                    WebrtcTemplate { id: monitor_id }.into_response()
                }),
            )
            .route(
                "/webrtc/:id/sdp",
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
            );
    }

    app = app.layer(cors);

    axum::Server::bind(&"0.0.0.0:9000".parse().unwrap())
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

pub(super) fn start(ctx: HttpServerContext) {
    tokio::spawn(async move {
        if let Err(e) = http_server(ctx).await {
            tracing::error!(?e, "HTTP server failed");
        }
    });
}
