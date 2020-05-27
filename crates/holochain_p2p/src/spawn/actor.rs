use crate::{actor, actor::*, event::*};

use crate::types::*;
use futures::future::FutureExt;
use holo_hash::*;
use std::sync::Arc;

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
                let res_fut = match self.handle_incoming_request(space, agent, data) {
                    Err(e) => {
                        let _ = respond(Err(kitsune_p2p::KitsuneP2pError::custom(e)));
                        return Ok(async move { Ok(()) }.boxed().into());
                    }
                    Ok(f) => f,
                };
                tokio::task::spawn(async move {
                    let _ = respond(res_fut.await.map_err(kitsune_p2p::KitsuneP2pError::custom));
                });
            }
            _ => (),
        }
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_incoming_request(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        agent: Arc<kitsune_p2p::KitsuneAgent>,
        _data: Arc<Vec<u8>>,
    ) -> HolochainP2pHandlerResult<Arc<Vec<u8>>> {
        let _space = DnaHash::from_kitsune(&space);
        let _agent = AgentPubKey::from_kitsune(&agent);
        // TODO - translate this to holochain types - i.e. `call_remote`
        //let mut evt_sender = self.evt_sender.clone();
        //Ok(async move { evt_sender.call_remote(space, agent).await.map(Arc::new) }.boxed().into())
        Ok(async move { Ok(Arc::new(vec![])) }.boxed().into())
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

    fn handle_call_remote(&mut self, _input: actor::CallRemote) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
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
