use anyhow::Result;
use holochain::conductor::ConductorHandle;
use holochain_websocket::WebsocketReceiver;
use holochain_websocket::WebsocketSender;

pub async fn admin_port(conductor: &ConductorHandle) -> u16 {
    conductor
        .get_arbitrary_admin_websocket_port()
        .await
        .expect("No admin port open on conductor")
}

pub async fn websocket_client(
    conductor: &ConductorHandle,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let port = admin_port(conductor).await;
    Ok(websocket_client_by_port(port).await?)
}

pub use holochain::sweettest::websocket_client_by_port;
