use anyhow::Result;
use tokio::io::AsyncWriteExt;

use crate::get_app;

async fn tcp_server() -> Result<()> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9866").await?;

    loop {
        let (mut socket, addr) = listener.accept().await?;

        tracing::info!(?addr, "New TCP connection");

        tokio::spawn(async move {
            socket.set_nodelay(true).ok();

            let mut data_rx = if let Some(monitor) = get_app().monitors().get(&0) {
                monitor.encoded_tx.subscribe()
            } else {
                tracing::error!("Monitor 0 not found");
                return;
            };

            while let Ok(sample) = data_rx.recv().await {
                let data = &sample.data[..];
                
                if let Err(e) = socket.write_all(data).await {
                    tracing::error!(?e, "Failed to write to connection");
                    break;
                }
            }
        });
    }
}

pub fn start() {
    tokio::spawn(async {
        if let Err(e) = tcp_server().await {
            tracing::error!(?e, "TCP server failed",);
        }
    });
}
