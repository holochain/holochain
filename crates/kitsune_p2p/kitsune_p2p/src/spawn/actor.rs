use crate::{actor, actor::*, event::*, types::*};
use futures::future::FutureExt;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

mod space;
use space::*;

/// if the user specifies zero (0) for remote_agent_count
const DEFAULT_BROADCAST_REMOTE_AGENT_COUNT: u8 = 5;

/// if the user specifies zero (0) for timeout_ms
const DEFAULT_BROADCAST_TIMEOUT_MS: u64 = 1000;

ghost_actor::ghost_chan! {
    pub(crate) chan Internal<crate::KitsuneP2pError> {
        /// Make a remote request right-now if we have an open connection,
        /// otherwise, return an error.
        fn immediate_request(space: Arc<KitsuneSpace>, agent: Arc<KitsuneAgent>, data: Arc<Vec<u8>>) -> Vec<u8>;

        /// Prune space if the agent count it is handling has dropped to zero.
        fn check_prune_space(space: Arc<KitsuneSpace>) -> ();

        /// List online agents that claim to be covering a basis hash
        fn list_online_agents_for_basis_hash(space: Arc<KitsuneSpace>, basis: Arc<KitsuneBasis>) -> Vec<Arc<KitsuneAgent>>;
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
                return Err(KitsuneP2pError::RoutingSpaceError(space));
            }
            Some(space) => space,
        };
        let space_request_fut = space.handle_internal_immediate_request(agent, data)?;
        Ok(async move { space_request_fut.await }.boxed().into())
    }

    fn handle_check_prune_space(
        &mut self,
        space: Arc<KitsuneSpace>,
    ) -> KitsuneP2pHandlerResult<()> {
        if let std::collections::hash_map::Entry::Occupied(entry) = self.spaces.entry(space) {
            if entry.get().len() == 0 {
                entry.remove();
            }
        }
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_list_online_agents_for_basis_hash(
        &mut self,
        space: Arc<KitsuneSpace>,
        // during short-circuit / full-sync mode,
        // we're ignoring the basis_hash and just returning everyone.
        _basis: Arc<KitsuneBasis>,
    ) -> KitsuneP2pHandlerResult<Vec<Arc<KitsuneAgent>>> {
        let space = match self.spaces.get_mut(&space) {
            None => {
                return Err(KitsuneP2pError::RoutingSpaceError(space));
            }
            Some(space) => space,
        };
        let res = space.list_agents();
        Ok(async move { Ok(res) }.boxed().into())
    }

    /// actual logic for handle_broadcast ...
    /// the top-level handler may or may not spawn a task for this
    fn handle_broadcast_inner(&mut self, input: actor::Broadcast) -> KitsuneP2pHandlerResult<u8> {
        let actor::Broadcast {
            space,
            basis,
            // ignore remote_agent_count for now - broadcast to everyone
            remote_agent_count: _,
            timeout_ms,
            broadcast,
        } = input;

        let timeout_ms = timeout_ms.expect("set by handle_broadcast");

        if !self.spaces.contains_key(&space) {
            return Err(KitsuneP2pError::RoutingSpaceError(space));
        }

        // encode the data to send
        let broadcast = Arc::new(wire::Wire::broadcast(broadcast).encode());

        let mut internal_sender = self.internal_sender.clone();

        // check 5(ish) times but with sane min/max
        const CHECK_COUNT: u64 = 5;
        let mut check_interval = timeout_ms / CHECK_COUNT;
        if check_interval < 10 {
            check_interval = 10;
        }
        if check_interval > timeout_ms {
            check_interval = timeout_ms;
        }

        Ok(async move {
            let start = std::time::Instant::now();
            let mut sent_to: HashSet<Arc<KitsuneAgent>> = HashSet::new();
            let send_success_count = Arc::new(std::sync::atomic::AtomicU8::new(0));

            loop {
                if let Ok(agent_list) = internal_sender
                    .ghost_actor_internal()
                    .list_online_agents_for_basis_hash(space.clone(), basis.clone())
                    .await
                {
                    for agent in agent_list {
                        if !sent_to.contains(&agent) {
                            sent_to.insert(agent.clone());
                            // send the broadcast here - but spawn
                            // so we're not holding up this loop
                            let mut internal_sender = internal_sender.clone();
                            let space = space.clone();
                            let broadcast = broadcast.clone();
                            let send_success_count = send_success_count.clone();
                            tokio::task::spawn(async move {
                                if let Ok(_) = internal_sender
                                    .ghost_actor_internal()
                                    .immediate_request(space, agent, broadcast)
                                    .await
                                {
                                    send_success_count
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                }
                            });
                        }
                    }
                }
                if (start.elapsed().as_millis() as u64) >= timeout_ms {
                    break;
                }
                tokio::time::delay_for(std::time::Duration::from_millis(check_interval)).await;
            }
            Ok(send_success_count.load(std::sync::atomic::Ordering::Relaxed))
        }
        .boxed()
        .into())
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
        let kspace = space.clone();
        let space = match self.spaces.get_mut(&space) {
            None => return Ok(async move { Ok(()) }.boxed().into()),
            Some(space) => space,
        };
        let space_leave_fut = space.handle_leave(agent)?;
        let mut internal_sender = self.internal_sender.clone();
        Ok(async move {
            space_leave_fut.await?;
            internal_sender
                .ghost_actor_internal()
                .check_prune_space(kspace)
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_request(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        data: Vec<u8>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space = match self.spaces.get_mut(&space) {
            None => {
                return Err(KitsuneP2pError::RoutingSpaceError(space));
            }
            Some(space) => space,
        };

        // encode the data to send
        let data = wire::Wire::request(data).encode();

        let space_request_fut = space.handle_request(agent, Arc::new(data))?;

        Ok(async move { space_request_fut.await }.boxed().into())
    }

    fn handle_broadcast(&mut self, mut input: actor::Broadcast) -> KitsuneP2pHandlerResult<u8> {
        // if the user doesn't care about remote_agent_count, apply default
        match input.remote_agent_count {
            None | Some(0) => {
                input.remote_agent_count = Some(DEFAULT_BROADCAST_REMOTE_AGENT_COUNT);
            }
            _ => (),
        }

        // if the user doesn't care about timeout_ms, apply default
        // also - if set to 0, we want to return immediately, but
        // spawn a task with that default timeout.
        let do_spawn = match input.timeout_ms {
            None | Some(0) => {
                input.timeout_ms = Some(DEFAULT_BROADCAST_TIMEOUT_MS);
                true
            }
            _ => false,
        };

        // gather the inner future
        let inner_fut = match self.handle_broadcast_inner(input) {
            Err(e) => return Err(e),
            Ok(f) => f,
        };

        // either spawn or return the future depending on timeout_ms logic
        if do_spawn {
            tokio::task::spawn(inner_fut);
            Ok(async move { Ok(0) }.boxed().into())
        } else {
            Ok(inner_fut)
        }
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
            Internal::CheckPruneSpace {
                span,
                respond,
                space,
            } => {
                let _g = span.enter();
                let res_fut = match self.handle_check_prune_space(space) {
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
            Internal::ListOnlineAgentsForBasisHash {
                span,
                respond,
                space,
                basis,
            } => {
                let _g = span.enter();
                let res_fut = match self.handle_list_online_agents_for_basis_hash(space, basis) {
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
