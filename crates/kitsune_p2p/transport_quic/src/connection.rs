use futures::{future::FutureExt, stream::StreamExt};
use kitsune_p2p_types::{
    dependencies::{ghost_actor, url2::*},
    transport::transport_connection::*,
    transport::*,
};

ghost_actor::ghost_chan! {
    Visibility(),
    Name(ConnectionInner),
    Error(TransportError),
    Api {
        PublishIncoming(
            "we received an incoming request - publish it",
            (Url2, Vec<u8>),
            Vec<u8>,
        ),
    }
}

/// QUIC implementation of kitsune TransportConnection actor.
struct TransportConnectionQuic {
    #[allow(dead_code)]
    internal_sender: TransportConnectionInternalSender<ConnectionInner>,
    quinn_connection: quinn::Connection,
    #[allow(dead_code)]
    incoming_sender: futures::channel::mpsc::Sender<TransportConnectionEvent>,
}

impl TransportConnectionHandler<(), ConnectionInner> for TransportConnectionQuic {
    fn handle_remote_url(&mut self) -> TransportConnectionHandlerResult<Url2> {
        let out = url2!(
            "{}:{}",
            crate::SCHEME,
            self.quinn_connection.remote_address()
        );
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_request(&mut self, input: Vec<u8>) -> TransportConnectionHandlerResult<Vec<u8>> {
        Ok(async move { Ok(input) }.boxed().into())
    }

    fn handle_ghost_actor_internal(
        &mut self,
        input: ConnectionInner,
    ) -> TransportConnectionResult<()> {
        match input {
            ConnectionInner::PublishIncoming(ghost_actor::ghost_chan::GhostChanItem {
                input,
                respond,
                ..
            }) => {
                let mut send_clone = self.incoming_sender.clone();
                tokio::task::spawn(async move {
                    let _ = respond(send_clone.incoming_request(input).await);
                });
            }
        }
        Ok(())
    }
}

/// Spawn a new QUIC TransportConnectionSender.
pub(crate) async fn spawn_transport_connection_quic(
    maybe_con: quinn::Connecting,
) -> TransportConnectionResult<(TransportConnectionSender, TransportConnectionEventReceiver)> {
    let con = maybe_con.await.map_err(TransportError::custom)?;
    let quinn::NewConnection {
        connection,
        mut bi_streams,
        ..
    } = con;
    let (incoming_sender, receiver) = futures::channel::mpsc::channel(10);
    let (sender, driver) =
        TransportConnectionSender::ghost_actor_spawn(Box::new(|internal_sender| {
            async move {
                let internal_sender_clone = internal_sender.clone();
                tokio::task::spawn(async move {
                    while let Some(Ok((mut bi_send, bi_recv))) = bi_streams.next().await {
                        let mut internal_sender_clone = internal_sender_clone.clone();
                        tokio::task::spawn(async move {
                            let req_data = match bi_recv.read_to_end(std::usize::MAX).await {
                                Err(_) => {
                                    // TODO - log?
                                    return;
                                }
                                Ok(data) => data,
                            };

                            let url = match internal_sender_clone.remote_url().await {
                                Err(_) => {
                                    // TODO - log?
                                    return;
                                }
                                Ok(url) => url,
                            };

                            let res_data = match internal_sender_clone
                                .ghost_actor_internal()
                                .publish_incoming((url, req_data))
                                .await
                            {
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

                Ok(TransportConnectionQuic {
                    internal_sender,
                    quinn_connection: connection,
                    incoming_sender,
                })
            }
            .boxed()
            .into()
        }))
        .await?;
    tokio::task::spawn(driver);
    Ok((sender, receiver))
}
