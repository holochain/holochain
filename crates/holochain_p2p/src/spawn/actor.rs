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
            Broadcast {
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
                    // this is a request type, not a broadcast
                    crate::wire::WireMessage::CallRemote { .. } => {
                        return Err(HolochainP2pError::invalid_p2p_message(
                            "invalid: call_remote is a request type, not a broadcast".to_string(),
                        ))
                    }
                    crate::wire::WireMessage::Publish {
                        from_agent,
                        request_validation_receipt,
                        entry_hash,
                        ops,
                    } => {
                        let res_fut = match self.handle_incoming_publish(
                            space,
                            agent,
                            from_agent,
                            request_validation_receipt,
                            entry_hash,
                            ops,
                        ) {
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
                    // this is a request type, not a broadcast
                    crate::wire::WireMessage::ValidationReceipt { .. } => {
                        return Err(HolochainP2pError::invalid_p2p_message(
                            "invalid: validation_receipt is a request type, not a broadcast"
                                .to_string(),
                        ))
                    }
                }
            }
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
                    // holochain_p2p never publishes via request
                    // these only occur on broadcasts
                    crate::wire::WireMessage::Publish { .. } => {
                        return Err(HolochainP2pError::invalid_p2p_message(
                            "invalid: publish is a broadcast type, not a request".to_string(),
                        ))
                    }
                    crate::wire::WireMessage::ValidationReceipt { receipt } => {
                        let res_fut =
                            match self.handle_incoming_validation_receipt(space, agent, receipt) {
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

    /// receiving an incoming publish from a remote node
    fn handle_incoming_publish(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        from_agent: AgentPubKey,
        request_validation_receipt: bool,
        entry_hash: holochain_types::composite_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> HolochainP2pHandlerResult<()> {
        let mut evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .publish(
                    dna_hash,
                    to_agent,
                    from_agent,
                    request_validation_receipt,
                    entry_hash,
                    ops,
                )
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    /// receiving an incoming validation receipt from a remote node
    fn handle_incoming_validation_receipt(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        receipt: Vec<u8>,
    ) -> HolochainP2pHandlerResult<()> {
        let receipt: SerializedBytes = UnsafeBytes::from(receipt).into();
        let mut evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .validation_receipt_received(dna_hash, agent_pub_key, receipt)
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

    fn handle_publish(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        request_validation_receipt: bool,
        entry_hash: holochain_types::composite_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
        timeout_ms: Option<u64>,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let basis = entry_hash.to_kitsune();

        let broadcast = crate::wire::WireMessage::publish(
            from_agent,
            request_validation_receipt,
            entry_hash,
            ops,
        )
        .encode()?;

        let mut kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            kitsune_p2p
                .broadcast(kitsune_p2p::actor::Broadcast {
                    space,
                    basis,
                    remote_agent_count: None, // default best-effort
                    timeout_ms,
                    broadcast,
                })
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_get_validation_package(
        &mut self,
        _input: actor::GetValidationPackage,
    ) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get(
        &mut self,
        _dna_hash: DnaHash,
        _entry_hash: holochain_types::composite_hash::AnyDhtHash,
        _options: actor::GetOptions,
    ) -> HolochainP2pHandlerResult<Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>>
    {
        Ok(async move { Ok(vec![]) }.boxed().into())
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

        let req = crate::wire::WireMessage::validation_receipt(receipt).encode()?;

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
