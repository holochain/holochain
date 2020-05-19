use futures::future::FutureExt;
use kitsune_p2p_types::{
    dependencies::url2::*, transport::transport_connection::*, transport::transport_listener::*,
};

/// QUIC implementation of kitsune TransportListener actor.
struct TransportListenerQuic {
    #[allow(dead_code)]
    internal_sender: TransportListenerInternalSender<()>,
}

impl TransportListenerHandler<(), ()> for TransportListenerQuic {
    fn handle_connect(
        &mut self,
        _input: Url2,
    ) -> TransportListenerHandlerResult<TransportConnectionSender> {
        Ok(async move {
            let (connection, _) = crate::connection::spawn_transport_connection_quic().await?;
            Ok(connection)
        }
        .boxed()
        .into())
    }
}

/// Spawn a new QUIC TransportListenerSender.
pub async fn spawn_transport_listener_quic(
) -> TransportListenerResult<(TransportListenerSender, TransportListenerEventReceiver)> {
    let (_sender, receiver) = tokio::sync::mpsc::channel(10);
    let (sender, driver) =
        TransportListenerSender::ghost_actor_spawn(Box::new(|internal_sender| {
            async move { Ok(TransportListenerQuic { internal_sender }) }
                .boxed()
                .into()
        }))
        .await?;
    tokio::task::spawn(driver);
    Ok((sender, receiver))
}
