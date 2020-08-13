// this is largely a passthrough that routes to a specific space handler

use crate::{actor, actor::*, event::*, types::*};
use futures::future::FutureExt;
use kitsune_p2p_types::async_lazy::AsyncLazy;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

mod gossip;
mod space;
use ghost_actor::dependencies::tracing;
use space::*;

ghost_actor::ghost_chan! {
    pub(crate) chan Internal<crate::KitsuneP2pError> {
        /// Register space event handler
        fn register_space_event_handler(recv: futures::channel::mpsc::Receiver<KitsuneP2pEvent>) -> ();
    }
}

pub(crate) struct KitsuneP2pActor {
    channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
    #[allow(dead_code)]
    internal_sender: ghost_actor::GhostSender<Internal>,
    #[allow(dead_code)]
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    spaces: HashMap<Arc<KitsuneSpace>, AsyncLazy<ghost_actor::GhostSender<KitsuneP2p>>>,
}

impl KitsuneP2pActor {
    pub fn new(
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        internal_sender: ghost_actor::GhostSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) -> KitsuneP2pResult<Self> {
        Ok(Self {
            channel_factory,
            internal_sender,
            evt_sender,
            spaces: HashMap::new(),
        })
    }
}

impl ghost_actor::GhostControlHandler for KitsuneP2pActor {}

impl ghost_actor::GhostHandler<Internal> for KitsuneP2pActor {}

impl InternalHandler for KitsuneP2pActor {
    fn handle_register_space_event_handler(
        &mut self,
        recv: futures::channel::mpsc::Receiver<KitsuneP2pEvent>,
    ) -> InternalHandlerResult<()> {
        let f = self.channel_factory.attach_receiver(recv);
        Ok(async move {
            f.await?;
            Ok(())
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for KitsuneP2pActor {}

impl KitsuneP2pEventHandler for KitsuneP2pActor {
    fn handle_call(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        Ok(self.evt_sender.call(space, agent, payload))
    }

    fn handle_notify(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.notify(space, agent, payload))
    }

    fn handle_gossip(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        op_hash: Arc<KitsuneOpHash>,
        op_data: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.gossip(space, agent, op_hash, op_data))
    }

    fn handle_fetch_op_hashes_for_constraints(
        &mut self,
        input: FetchOpHashesForConstraintsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<Arc<KitsuneOpHash>>> {
        Ok(self.evt_sender.fetch_op_hashes_for_constraints(input))
    }

    fn handle_fetch_op_hash_data(
        &mut self,
        input: FetchOpHashDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
        Ok(self.evt_sender.fetch_op_hash_data(input))
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        Ok(self.evt_sender.sign_network_data(input))
    }
}

impl ghost_actor::GhostHandler<KitsuneP2p> for KitsuneP2pActor {}

impl KitsuneP2pHandler for KitsuneP2pActor {
    fn handle_join(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let internal_sender = self.internal_sender.clone();
        let space2 = space.clone();
        let space_sender = match self.spaces.entry(space.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(AsyncLazy::new(async move {
                let (send, evt_recv) = spawn_space(space2)
                    .await
                    .expect("cannot fail to create space");
                internal_sender
                    .register_space_event_handler(evt_recv)
                    .await
                    .expect("FAIL");
                send
            })),
        };
        let space_sender = space_sender.get();
        Ok(async move { space_sender.await.join(space, agent).await }
            .boxed()
            .into())
    }

    fn handle_leave(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Ok(async move { Ok(()) }.boxed().into()),
            Some(space) => space.get(),
        };
        Ok(async move {
            space_sender.await.leave(space.clone(), agent).await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_rpc_single(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(
            async move { space_sender.await.rpc_single(space, agent, payload).await }
                .boxed()
                .into(),
        )
    }

    #[tracing::instrument(skip(self, input))]
    fn handle_rpc_multi(
        &mut self,
        input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        let space_sender = match self.spaces.get_mut(&input.space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(input.space)),
            Some(space) => space.get(),
        };
        Ok(async move { space_sender.await.rpc_multi(input).await }
            .boxed()
            .into())
    }

    fn handle_notify_multi(&mut self, input: actor::NotifyMulti) -> KitsuneP2pHandlerResult<u8> {
        let space_sender = match self.spaces.get_mut(&input.space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(input.space)),
            Some(space) => space.get(),
        };
        Ok(async move { space_sender.await.notify_multi(input).await }
            .boxed()
            .into())
    }
}
