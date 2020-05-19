use futures::future::FutureExt;
use kitsune_p2p_types::transport::transport_connection::*;

/// QUIC implementation of kitsune TransportConnection actor.
struct TransportConnectionQuic {
    #[allow(dead_code)]
    internal_sender: TransportConnectionInternalSender<()>,
}

impl TransportConnectionHandler<(), ()> for TransportConnectionQuic {
    fn handle_request(&mut self, input: Vec<u8>) -> TransportConnectionHandlerResult<Vec<u8>> {
        Ok(async move { Ok(input) }.boxed().into())
    }
}

/// Spawn a new QUIC TransportConnectionSender.
pub(crate) async fn spawn_transport_connection_quic(
) -> TransportConnectionResult<(TransportConnectionSender, TransportConnectionEventReceiver)> {
    let (_sender, receiver) = tokio::sync::mpsc::channel(10);
    let (sender, driver) =
        TransportConnectionSender::ghost_actor_spawn(Box::new(|internal_sender| {
            async move { Ok(TransportConnectionQuic { internal_sender }) }
                .boxed()
                .into()
        }))
        .await?;
    tokio::task::spawn(driver);
    Ok((sender, receiver))
}
