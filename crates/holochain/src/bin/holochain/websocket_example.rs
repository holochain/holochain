use crate::Port;
use holochain_2020::conductor::api::{AdminResponse, ConductorResponse};
use holochain_websocket::*;
use std::convert::TryInto;
use std::sync::Arc;
use tokio::stream::StreamExt;
use tracing::*;
use url2::prelude::*;

// TODO: use actual error type
type StdResult<T = ()> = Result<T, Box<dyn std::error::Error + Sync + Send>>;

pub async fn websocket_example(port: Port) -> StdResult {
    let mut listener = websocket_bind(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?;
    debug!("LISTENING AT: {}", listener.local_addr());
    let mut listener_handles = Vec::new();
    while let Some(maybe_con) = listener.next().await {
        let (_, recv_socket) = maybe_con.await?;
        listener_handles.push(tokio::task::spawn(recv_msgs(recv_socket)));
    }
    for h in listener_handles {
        h.await??;
    }
    Ok(())
}
async fn recv_msgs(mut recv_socket: WebsocketReceiver) -> StdResult {
    info!("CONNECTION: {}", recv_socket.remote_addr());

    while let Some(msg) = recv_socket.next().await {
        match msg {
            WebsocketMessage::Request(msg, response) => {
                response(
                    ConductorResponse::AdminResponse {
                        response: Box::new(AdminResponse::DnaAdded),
                    }
                    .try_into()?,
                )
                .await?;
            }
            msg => {
                debug!("Other message: {:?}", msg);
                break;
            }
        }
    }
    Ok(())
}
