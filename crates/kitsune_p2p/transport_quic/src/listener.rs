use futures::{future::FutureExt, stream::StreamExt};
use kitsune_p2p_types::{
    dependencies::{ghost_actor, url2::*},
    transport::transport_connection::*,
    transport::transport_listener::*,
    transport::*,
};

ghost_actor::ghost_chan! {
    Visibility(),
    Name(ListenerInner),
    Error(TransportError),
    Api {
        RegisterIncoming(
            "our incoming task has produced a connection instance",
            (TransportConnectionSender, TransportConnectionEventReceiver),
            (),
        ),
    }
}

/// QUIC implementation of kitsune TransportListener actor.
struct TransportListenerQuic {
    #[allow(dead_code)]
    internal_sender: TransportListenerInternalSender<ListenerInner>,
    quinn_endpoint: quinn::Endpoint,
    incoming_sender: futures::channel::mpsc::Sender<TransportListenerEvent>,
}

impl TransportListenerHandler<(), ListenerInner> for TransportListenerQuic {
    fn handle_connect(
        &mut self,
        input: Url2,
    ) -> TransportListenerHandlerResult<(TransportConnectionSender, TransportConnectionEventReceiver)>
    {
        // TODO fix this block_on
        let addr = tokio_safe_block_on::tokio_safe_block_on(
            crate::url_to_addr(&input, crate::SCHEME),
            std::time::Duration::from_secs(1),
        )
        .unwrap()?;
        let maybe_con = self
            .quinn_endpoint
            .connect(&addr, "")
            .map_err(TransportError::custom)?;
        Ok(
            async move { crate::connection::spawn_transport_connection_quic(maybe_con).await }
                .boxed()
                .into(),
        )
    }

    fn handle_ghost_actor_internal(&mut self, input: ListenerInner) -> TransportListenerResult<()> {
        match input {
            ListenerInner::RegisterIncoming(ghost_actor::ghost_chan::GhostChanItem {
                input,
                respond,
                ..
            }) => {
                let mut send_clone = self.incoming_sender.clone();
                tokio::task::spawn(async move {
                    let _ = respond(send_clone.incoming_connection(input).await);
                });
            }
        }
        Ok(())
    }
}

/// Spawn a new QUIC TransportListenerSender.
pub async fn spawn_transport_listener_quic(
    bind_to: Url2,
) -> TransportListenerResult<(TransportListenerSender, TransportListenerEventReceiver)> {
    let (quinn_endpoint, mut incoming) = quinn::Endpoint::builder()
        .bind(&crate::url_to_addr(&bind_to, crate::SCHEME).await?)
        .map_err(TransportError::custom)?;

    let (incoming_sender, receiver) = futures::channel::mpsc::channel(10);
    let (sender, driver) =
        TransportListenerSender::ghost_actor_spawn(Box::new(|internal_sender| {
            async move {
                let internal_sender_clone = internal_sender.clone();
                tokio::task::spawn(async move {
                    while let Some(maybe_con) = incoming.next().await {
                        let mut internal_sender_clone = internal_sender_clone.clone();

                        // TODO - some buffer_unordered(10) magic
                        //        so we don't process infinite incoming connections
                        tokio::task::spawn(async move {
                            let r =
                                match crate::connection::spawn_transport_connection_quic(maybe_con)
                                    .await
                                {
                                    Err(_) => {
                                        // TODO - log this?
                                        return;
                                    }
                                    Ok(r) => r,
                                };

                            if let Err(_) = internal_sender_clone
                                .ghost_actor_internal()
                                .register_incoming(r)
                                .await
                            {
                                // TODO - log this?
                                return;
                            }
                        });
                    }
                });

                Ok(TransportListenerQuic {
                    internal_sender,
                    quinn_endpoint,
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
