use futures::{future::FutureExt, stream::StreamExt};
use kitsune_p2p_types::{
    dependencies::{ghost_actor, url2::*},
    transport::transport_connection::*,
    transport::*,
};

ghost_actor::ghost_actor! {
    actor ConnectionInner<TransportError> {
        /// we received an incoming request - publish it
        fn publish_incoming(url: Url2, data: Vec<u8>) -> Vec<u8>;
    }
}

/// QUIC implementation of kitsune TransportConnection actor.
struct TransportConnectionQuic {
    #[allow(dead_code)]
    internal_sender: ghost_actor::GhostSender<ConnectionInner>,
    quinn_connection: quinn::Connection,
    #[allow(dead_code)]
    incoming_sender: futures::channel::mpsc::Sender<TransportConnectionEvent>,
}

impl ghost_actor::GhostControlHandler for TransportConnectionQuic {}

impl ghost_actor::GhostHandler<ConnectionInner> for TransportConnectionQuic {}

impl ConnectionInnerHandler for TransportConnectionQuic {
    fn handle_publish_incoming(
        &mut self,
        url: Url2,
        data: Vec<u8>,
    ) -> ConnectionInnerHandlerResult<Vec<u8>> {
        let send_clone = self.incoming_sender.clone();
        Ok(async move { send_clone.incoming_request(url, data).await }
            .boxed()
            .into())
    }
}

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
            let (mut bi_send, bi_recv) = maybe_bi.await.map_err(TransportError::custom)?;
            bi_send
                .write_all(&input)
                .await
                .map_err(TransportError::custom)?;
            bi_send.finish().await.map_err(TransportError::custom)?;
            let res = bi_recv
                .read_to_end(std::usize::MAX)
                .await
                .map_err(TransportError::custom)?;
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
    let con = maybe_con.await.map_err(TransportError::custom)?;
    let quinn::NewConnection {
        connection,
        mut bi_streams,
        ..
    } = con;
    let (incoming_sender, receiver) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let internal_sender = builder
        .channel_factory()
        .create_channel::<ConnectionInner>()
        .await?;

    let sender = builder
        .channel_factory()
        .create_channel::<TransportConnection>()
        .await?;

    let internal_sender_clone = internal_sender.clone();
    let sender_clone = sender.clone();
    tokio::task::spawn(async move {
        while let Some(Ok((mut bi_send, bi_recv))) = bi_streams.next().await {
            let internal_sender_clone = internal_sender_clone.clone();
            let sender_clone = sender_clone.clone();
            tokio::task::spawn(async move {
                let req_data = match bi_recv.read_to_end(std::usize::MAX).await {
                    Err(_) => {
                        // TODO - log?
                        return;
                    }
                    Ok(data) => data,
                };

                let url = match sender_clone.remote_url().await {
                    Err(_) => {
                        // TODO - log?
                        return;
                    }
                    Ok(url) => url,
                };

                let res_data = match internal_sender_clone.publish_incoming(url, req_data).await {
                    Err(_) => {
                        // TODO - log?
                        return;
                    }
                    Ok(data) => data,
                };

                if let Err(_) = bi_send.write_all(&res_data).await {
                    // TODO - log?
                }

                if let Err(_) = bi_send.finish().await {
                    // TODO - log?
                }
            });
        }
    });

    let actor = TransportConnectionQuic {
        internal_sender,
        quinn_connection: connection,
        incoming_sender,
    };
    tokio::task::spawn(builder.spawn(actor));

    Ok((sender, receiver))
}
