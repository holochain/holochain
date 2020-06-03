use crate::{actor::*, event::*, *};

use futures::future::FutureExt;

use crate::types::AgentPubKeyExt;

ghost_actor::ghost_chan! {
    pub(crate) chan Internal<crate::HolochainP2pError> {
        /// channel for handling incoming kitsune p2p events
        fn kitsune_p2p_event(event: kitsune_p2p::event::KitsuneP2pEvent) -> ();
    }
}

pub(crate) struct HolochainP2pActor {
    #[allow(dead_code)]
    internal_sender: HolochainP2pInternalSender<Internal>,
    #[allow(dead_code)]
    evt_sender: futures::channel::mpsc::Sender<HolochainP2pEvent>,
    kitsune_p2p: kitsune_p2p::actor::KitsuneP2pSender,
}

impl HolochainP2pActor {
    /// constructor
    pub async fn new(
        internal_sender: HolochainP2pInternalSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<HolochainP2pEvent>,
    ) -> HolochainP2pResult<Self> {
        let (kitsune_p2p, mut kitsune_p2p_events) = kitsune_p2p::spawn_kitsune_p2p().await?;

        let mut internal_sender_clone = internal_sender.clone();
        tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(event) = kitsune_p2p_events.next().await {
                if let Err(e) = internal_sender_clone
                    .ghost_actor_internal()
                    .kitsune_p2p_event(event)
                    .await
                {
                    ghost_actor::dependencies::tracing::error!(error = ?e);
                }
            }
        });

        Ok(Self {
            internal_sender,
            evt_sender,
            kitsune_p2p,
        })
    }

    /// ghost actor glue that translates kitsune events into local handlers (step 2)
    fn handle_internal_kitsune_p2p_event(
        &mut self,
        event: kitsune_p2p::event::KitsuneP2pEvent,
    ) -> HolochainP2pHandlerResult<()> {
        use kitsune_p2p::event::KitsuneP2pEvent::*;
        match event {
            Request {
                span,
                respond,
                space,
                agent,
                data,
            } => {
                let _g = span.enter();
                let space = DnaHash::from_kitsune(&space);
                let agent = AgentPubKey::from_kitsune(&agent);

                let request = crate::wire::WireMessage::decode(data)?;

                match request {
                    crate::wire::WireMessage::CallRemote { data } => {
                        let res_fut = match self.handle_incoming_call_remote(space, agent, data) {
                            Err(e) => {
                                let _ = respond(Err(e.into()));
                                return Ok(async move { Ok(()) }.boxed().into());
                            }
                            Ok(f) => f,
                        };
                        tokio::task::spawn(async move {
                            let _ = respond(res_fut.await.map_err(Into::into));
                        });
                    }
                    crate::wire::WireMessage::SendValidationReceipt { receipt } => {
                        let res_fut = match self
                            .handle_incoming_send_validation_receipt(space, agent, receipt)
                        {
                            Err(e) => {
                                let _ = respond(Err(e.into()));
                                return Ok(async move { Ok(()) }.boxed().into());
                            }
                            Ok(f) => f,
                        };
                        tokio::task::spawn(async move {
                            let _ = match res_fut.await {
                                Err(e) => respond(Err(e.into())),
                                // validation receipts don't need a response
                                // send back an empty vec for now
                                Ok(_) => respond(Ok(Vec::with_capacity(0))),
                            };
                        });
                    }
                }
            }
            _ => (),
        }
        Ok(async move { Ok(()) }.boxed().into())
    }

    /// receiving an incoming request from a remote node
    fn handle_incoming_call_remote(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        data: Vec<u8>,
    ) -> HolochainP2pHandlerResult<Vec<u8>> {
        let data: SerializedBytes = UnsafeBytes::from(data).into();
        let mut evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender.call_remote(dna_hash, agent_pub_key, data).await;
            res.map(|res| UnsafeBytes::from(res).into())
        }
        .boxed()
        .into())
    }

    /// receiving an incoming validation receipt from a remote node
    fn handle_incoming_send_validation_receipt(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        receipt: Vec<u8>,
    ) -> HolochainP2pHandlerResult<()> {
        let receipt: SerializedBytes = UnsafeBytes::from(receipt).into();
        let mut evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .send_validation_receipt(dna_hash, agent_pub_key, receipt)
                .await
        }
        .boxed()
        .into())
    }
}

impl HolochainP2pHandler<(), Internal> for HolochainP2pActor {
    fn handle_join(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let agent = agent_pub_key.into_kitsune();

        let mut kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move { Ok(kitsune_p2p.join(space, agent).await?) }
            .boxed()
            .into())
    }

    fn handle_leave(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let agent = agent_pub_key.into_kitsune();

        let mut kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move { Ok(kitsune_p2p.leave(space, agent).await?) }
            .boxed()
            .into())
    }

    fn handle_call_remote(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        request: SerializedBytes,
    ) -> HolochainP2pHandlerResult<SerializedBytes> {
        let space = dna_hash.into_kitsune();
        let agent = agent_pub_key.into_kitsune();

        let req = crate::wire::WireMessage::call_remote(request).encode()?;

        let mut kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            let result = kitsune_p2p.request(space, agent, req).await?;
            let result = UnsafeBytes::from(result).into();
            Ok(result)
        }
        .boxed()
        .into())
    }

    fn handle_publish(&mut self, _input: actor::Publish) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get_validation_package(
        &mut self,
        _input: actor::GetValidationPackage,
    ) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get(&mut self, _input: actor::Get) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get_links(&mut self, _input: actor::GetLinks) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_send_validation_receipt(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        receipt: SerializedBytes,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let agent = agent_pub_key.into_kitsune();

        let req = crate::wire::WireMessage::send_validation_receipt(receipt).encode()?;

        let mut kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            kitsune_p2p.request(space, agent, req).await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    /// ghost actor glue that translates kitsune events into local handlers (step 1)
    fn handle_ghost_actor_internal(&mut self, input: Internal) -> HolochainP2pResult<()> {
        match input {
            Internal::KitsuneP2pEvent {
                span,
                respond,
                event,
            } => {
                let _g = span.enter();
                let res_fut = match self.handle_internal_kitsune_p2p_event(event) {
                    Err(e) => {
                        let _ = respond(Err(e));
                        return Ok(());
                    }
                    Ok(f) => f,
                };
                tokio::task::spawn(async move {
                    let _ = respond(res_fut.await);
                });
            }
        }
        Ok(())
    }
}
