use futures::{future::FutureExt, stream::StreamExt};
use kitsune_p2p_types::{
    dependencies::{ghost_actor, url2::*},
    transport::transport_connection::*,
    transport::*,
};

/// QUIC implementation of kitsune TransportConnection actor.
struct TransportConnectionQuic {
    quinn_connection: quinn::Connection,
}

impl ghost_actor::GhostControlHandler for TransportConnectionQuic {}

impl ghost_actor::GhostHandler<TransportConnection> for TransportConnectionQuic {}

impl TransportConnectionHandler for TransportConnectionQuic {
    fn handle_remote_url(&mut self) -> TransportConnectionHandlerResult<Url2> {
        let out = url2!(
            "{}://{}",
            crate::SCHEME,
            self.quinn_connection.remote_address()
        );
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_request(&mut self, input: Vec<u8>) -> TransportConnectionHandlerResult<Vec<u8>> {
        let maybe_bi = self.quinn_connection.open_bi();
        Ok(async move {
            let (mut bi_send, bi_recv) = maybe_bi.await.map_err(TransportError::other)?;
            bi_send
                .write_all(&input)
                .await
                .map_err(TransportError::other)?;
            bi_send.finish().await.map_err(TransportError::other)?;
            let res = bi_recv
                .read_to_end(std::usize::MAX)
                .await
                .map_err(TransportError::other)?;
            Ok(res)
        }
        .boxed()
        .into())
    }
}

/// Spawn a new QUIC TransportConnectionSender.
pub(crate) async fn spawn_transport_connection_quic(
    maybe_con: quinn::Connecting,
) -> TransportConnectionResult<(
    ghost_actor::GhostSender<TransportConnection>,
    TransportConnectionEventReceiver,
)> {
    let con = maybe_con.await.map_err(TransportError::other)?;

    let quinn::NewConnection {
        connection,
        mut bi_streams,
        ..
    } = con;

    let (incoming_sender, receiver) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let sender = builder
        .channel_factory()
        .create_channel::<TransportConnection>()
        .await?;

    let sender_clone = sender.clone();
    tokio::task::spawn(async move {
        while let Some(Ok((mut bi_send, bi_recv))) = bi_streams.next().await {
            let sender_clone = sender_clone.clone();
            let incoming_sender = incoming_sender.clone();
            tokio::task::spawn(async move {
                let req_data = bi_recv
                    .read_to_end(std::usize::MAX)
                    .await
                    .map_err(TransportError::other)?;
                let url = sender_clone
                    .remote_url()
                    .await
                    .map_err(TransportError::other)?;

                let res_data = incoming_sender.incoming_request(url, req_data).await?;

                bi_send
                    .write_all(&res_data)
                    .await
                    .map_err(TransportError::other)?;

                bi_send.finish().await.map_err(TransportError::other)?;
                TransportResult::Ok(())
            });
        }
    });

    let actor = TransportConnectionQuic {
        quinn_connection: connection,
    };
    tokio::task::spawn(builder.spawn(actor));

    Ok((sender, receiver))
}
