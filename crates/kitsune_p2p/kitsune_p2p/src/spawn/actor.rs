use crate::{actor, actor::*, event::*, types::*};
use futures::future::FutureExt;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

mod space;
use space::*;

ghost_actor::ghost_chan! {
    pub(crate) chan Internal<crate::KitsuneP2pError> {
        /// Make a remote request right-now if we have an open connection,
        /// otherwise, return an error.
        fn immediate_request(space: Arc<KitsuneSpace>, agent: Arc<KitsuneAgent>, data: Arc<Vec<u8>>) -> Vec<u8>;
    }
}

pub(crate) struct KitsuneP2pActor {
    #[allow(dead_code)]
    internal_sender: KitsuneP2pInternalSender<Internal>,
    #[allow(dead_code)]
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    spaces: HashMap<Arc<KitsuneSpace>, Space>,
}

impl KitsuneP2pActor {
    pub fn new(
        internal_sender: KitsuneP2pInternalSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) -> KitsuneP2pResult<Self> {
        Ok(Self {
            internal_sender,
            evt_sender,
            spaces: HashMap::new(),
        })
    }

    fn handle_internal_immediate_request(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        data: Arc<Vec<u8>>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space = match self.spaces.get_mut(&space) {
            None => {
                return Err(KitsuneP2pError::RoutingFailure(format!(
                    "space '{:?}' not joined",
                    space
                )))
            }
            Some(space) => space,
        };
        let space_request_fut = space.handle_internal_immediate_request(agent, data)?;
        Ok(async move { space_request_fut.await }.boxed().into())
    }
}

impl KitsuneP2pHandler<(), Internal> for KitsuneP2pActor {
    fn handle_join(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space = match self.spaces.entry(space.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Space::new(
                space,
                self.internal_sender.clone(),
                self.evt_sender.clone(),
            )),
        };
        space.handle_join(agent)
    }

    fn handle_leave(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space = match self.spaces.get_mut(&space) {
            None => return Ok(async move { Ok(()) }.boxed().into()),
            Some(space) => space,
        };
        let space_leave_fut = space.handle_leave(agent)?;
        Ok(async move {
            space_leave_fut.await?;
            // TODO - clean up empty spaces
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_request(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        data: Arc<Vec<u8>>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space = match self.spaces.get_mut(&space) {
            None => {
                return Err(KitsuneP2pError::RoutingFailure(format!(
                    "space '{:?}' not joined",
                    space
                )))
            }
            Some(space) => space,
        };
        let space_request_fut = space.handle_request(agent, data)?;
        Ok(async move { space_request_fut.await }.boxed().into())
    }

    fn handle_broadcast(&mut self, _input: actor::Broadcast) -> KitsuneP2pHandlerResult<u32> {
        Ok(async move { Ok(0) }.boxed().into())
    }

    fn handle_multi_request(
        &mut self,
        _input: actor::MultiRequest,
    ) -> KitsuneP2pHandlerResult<Vec<actor::MultiRequestResponse>> {
        Ok(async move { Ok(vec![]) }.boxed().into())
    }

    fn handle_ghost_actor_internal(&mut self, input: Internal) -> KitsuneP2pResult<()> {
        match input {
            Internal::ImmediateRequest {
                span,
                respond,
                space,
                agent,
                data,
            } => {
                let _g = span.enter();
                let res_fut = match self.handle_internal_immediate_request(space, agent, data) {
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
