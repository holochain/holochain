use crate::{actor, actor::*, event::*, types::*};
use futures::future::FutureExt;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};
use kitsune_p2p_types::async_lazy::AsyncLazy;

mod space;
use space::*;

mod gossip;

/// if the user specifies None or zero (0) for remote_agent_count
const DEFAULT_NOTIFY_REMOTE_AGENT_COUNT: u8 = 5;

/// if the user specifies None or zero (0) for timeout_ms
const DEFAULT_NOTIFY_TIMEOUT_MS: u64 = 1000;

/// if the user specifies None or zero (0) for remote_agent_count
const DEFAULT_RPC_MULTI_REMOTE_AGENT_COUNT: u8 = 2;

/// if the user specifies None or zero (0) for timeout_ms
const DEFAULT_RPC_MULTI_TIMEOUT_MS: u64 = 1000;

/// if the user specifies None or zero (0) for race_timeout_ms
const DEFAULT_RPC_MULTI_RACE_TIMEOUT_MS: u64 = 200;

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
    channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
    #[allow(dead_code)]
    internal_sender: ghost_actor::GhostSender<Internal>,
    #[allow(dead_code)]
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    spaces: HashMap<Arc<KitsuneSpace>, Space>,
    gossip_loops: HashMap<Arc<KitsuneSpace>, AsyncLazy<bool>>,
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
            gossip_loops: HashMap::new(),
        })
    }

    /// actual logic for handle_rpc_multi ...
    /// the top-level handler may or may not spawn a task for this
    #[allow(unused_variables, unused_assignments, unused_mut)]
    fn handle_rpc_multi_inner(
        &mut self,
        input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        let actor::RpcMulti {
            space,
            from_agent,
            basis,
            remote_agent_count,
            timeout_ms,
            as_race,
            race_timeout_ms,
            payload,
        } = input;

        let remote_agent_count = remote_agent_count.expect("set by handle_rpc_multi");
        let timeout_ms = timeout_ms.expect("set by handle_rpc_multi");
        let mut race_timeout_ms = race_timeout_ms.expect("set by handle_rpc_multi");
        if !as_race {
            // if these are the same, the effect is that we are not racing
            race_timeout_ms = timeout_ms;
        }

        if !self.spaces.contains_key(&space) {
            return Err(KitsuneP2pError::RoutingSpaceError(space));
        }

        // encode the data to send
        let payload = Arc::new(wire::Wire::call(payload).encode());

        let mut internal_sender = self.internal_sender.clone();

        Ok(async move {
            let start = std::time::Instant::now();

            // TODO - this logic isn't quite right
            //        but we don't want to spend too much time on it
            //        when we don't have a real peer-discovery pathway
            //      - right now we're checking for enough agents up to
            //        the race_timeout - then stopping that and
            //        checking for responses.

            // send calls to agents
            let mut sent_to: HashSet<Arc<KitsuneAgent>> = HashSet::new();
            let (res_send, mut res_recv) = tokio::sync::mpsc::channel(10);
            loop {
                let mut i_s = internal_sender.clone();
                if let Ok(agent_list) = i_s
                    .list_online_agents_for_basis_hash(space.clone(), basis.clone())
                    .await
                {
                    for agent in agent_list {
                        // for each agent returned
                        // if we haven't sent them a call
                        // and they aren't the requestor - send a call
                        // if we meet our request quota break out.
                        if agent != from_agent && !sent_to.contains(&agent) {
                            sent_to.insert(agent.clone());
                            let mut i_s = internal_sender.clone();
                            let space = space.clone();
                            let payload = payload.clone();
                            let mut res_send = res_send.clone();
                            // make the call - the responses will be
                            // sent back to our channel
                            tokio::task::spawn(async move {
                                if let Ok(response) =
                                    i_s.immediate_request(space, agent.clone(), payload).await
                                {
                                    let _ = res_send
                                        .send(actor::RpcMultiResponse { agent, response })
                                        .await;
                                }
                            });
                        }
                        if sent_to.len() >= remote_agent_count as usize {
                            break;
                        }
                    }

                    // keep checking until we meet our call quota
                    // or we get to our race timeout
                    if sent_to.len() >= remote_agent_count as usize
                        || start.elapsed().as_millis() as u64 > race_timeout_ms
                    {
                        break;
                    }

                    // we haven't broken, but there are no new peers to send to
                    // wait for a bit, maybe more will come online
                    // NOTE - this logic is naive - fix once we have
                    //        a unified loop with the peer-discovery
                    tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
                }
            }

            // await responses
            let mut out = Vec::new();
            let mut result_fut = None;
            loop {
                // set up our future for waiting on results
                if result_fut.is_none() {
                    // if there are results already pending, pull them out
                    while let Ok(result) = res_recv.try_recv() {
                        out.push(result);
                    }

                    use tokio::stream::StreamExt;
                    result_fut = Some(res_recv.next());
                }

                // calculate the time to wait based on our barriers
                let elapsed = start.elapsed().as_millis() as u64;
                let mut time_remaining = if elapsed >= race_timeout_ms {
                    if elapsed < timeout_ms {
                        timeout_ms - elapsed
                    } else {
                        1
                    }
                } else {
                    race_timeout_ms - elapsed
                };
                if time_remaining < 1 {
                    time_remaining = 1;
                }

                // await either
                //  -  (LEFT) - we need to check one of our timeouts
                //  - (RIGHT) - we have received a response
                match futures::future::select(
                    tokio::time::delay_for(std::time::Duration::from_millis(time_remaining)),
                    result_fut.take().unwrap(),
                )
                .await
                {
                    futures::future::Either::Left((_, r_fut)) => {
                        result_fut = Some(r_fut);
                    }
                    futures::future::Either::Right((result, _)) => {
                        if result.is_none() {
                            ghost_actor::dependencies::tracing::error!("this should not happen");
                            break;
                        }
                        out.push(result.unwrap());
                    }
                }

                // break out if we are beyond time
                let elapsed = start.elapsed().as_millis() as u64;
                if elapsed > timeout_ms
                    || (elapsed > race_timeout_ms && out.len() >= remote_agent_count as usize)
                {
                    break;
                }
            }

            Ok(out)
        }
        .boxed()
        .into())
    }

    /// actual logic for handle_notify_multi ...
    /// the top-level handler may or may not spawn a task for this
    fn handle_notify_multi_inner(
        &mut self,
        input: actor::NotifyMulti,
    ) -> KitsuneP2pHandlerResult<u8> {
        let actor::NotifyMulti {
            space,
            basis,
            // ignore remote_agent_count for now - broadcast to everyone
            remote_agent_count: _,
            timeout_ms,
            payload,
        } = input;

        let timeout_ms = timeout_ms.expect("set by handle_notify_multi");

        if !self.spaces.contains_key(&space) {
            return Err(KitsuneP2pError::RoutingSpaceError(space));
        }

        // encode the data to send
        let payload = Arc::new(wire::Wire::notify(payload).encode());

        let internal_sender = self.internal_sender.clone();

        // check 5(ish) times but with sane min/max
        // FYI - this strategy will likely change when we are no longer
        //       purely short-circuit, and we are looping on peer discovery.
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
                    .list_online_agents_for_basis_hash(space.clone(), basis.clone())
                    .await
                {
                    for agent in agent_list {
                        if !sent_to.contains(&agent) {
                            sent_to.insert(agent.clone());
                            // send the notify here - but spawn
                            // so we're not holding up this loop
                            let internal_sender = internal_sender.clone();
                            let space = space.clone();
                            let payload = payload.clone();
                            let send_success_count = send_success_count.clone();
                            tokio::task::spawn(async move {
                                if let Ok(_) = internal_sender
                                    .immediate_request(space, agent, payload)
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

impl ghost_actor::GhostControlHandler for KitsuneP2pActor {}

impl ghost_actor::GhostHandler<Internal> for KitsuneP2pActor {}

impl InternalHandler for KitsuneP2pActor {
    fn handle_immediate_request(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        data: Arc<Vec<u8>>,
    ) -> InternalHandlerResult<Vec<u8>> {
        let space = match self.spaces.get_mut(&space) {
            None => {
                return Err(KitsuneP2pError::RoutingSpaceError(space));
            }
            Some(space) => space,
        };
        let space_request_fut = space.handle_internal_immediate_request(agent, data)?;
        Ok(async move { space_request_fut.await }.boxed().into())
    }

    fn handle_check_prune_space(&mut self, space: Arc<KitsuneSpace>) -> InternalHandlerResult<()> {
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
    ) -> InternalHandlerResult<Vec<Arc<KitsuneAgent>>> {
        let space = match self.spaces.get_mut(&space) {
            None => {
                return Err(KitsuneP2pError::RoutingSpaceError(space));
            }
            Some(space) => space,
        };
        let res = space.list_agents();
        Ok(async move { Ok(res) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<KitsuneP2p> for KitsuneP2pActor {}

impl KitsuneP2pHandler for KitsuneP2pActor {
    fn handle_join(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space = match self.spaces.entry(space.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let out = entry.insert(Space::new(
                    space.clone(),
                    self.internal_sender.clone(),
                    self.evt_sender.clone(),
                ));
                let _gossip_evt_recv = gossip::spawn_gossip_module(
                    space,
                ); //.await?;
                //self.channel_factory.attach_receiver(gossip_evt_recv);
                out
            }
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
        let internal_sender = self.internal_sender.clone();
        Ok(async move {
            space_leave_fut.await?;
            internal_sender.check_prune_space(kspace).await?;
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
        let space = match self.spaces.get_mut(&space) {
            None => {
                return Err(KitsuneP2pError::RoutingSpaceError(space));
            }
            Some(space) => space,
        };

        // encode the data to send
        let payload = wire::Wire::call(payload).encode();

        let space_request_fut = space.handle_rpc_single(agent, Arc::new(payload))?;

        Ok(async move { space_request_fut.await }.boxed().into())
    }

    fn handle_rpc_multi(
        &mut self,
        mut input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        // if the user doesn't care about remote_agent_count, apply default
        match input.remote_agent_count {
            None | Some(0) => {
                input.remote_agent_count = Some(DEFAULT_RPC_MULTI_REMOTE_AGENT_COUNT);
            }
            _ => (),
        }

        // if the user doesn't care about timeout_ms, apply default
        match input.timeout_ms {
            None | Some(0) => {
                input.timeout_ms = Some(DEFAULT_RPC_MULTI_TIMEOUT_MS);
            }
            _ => (),
        }

        // if the user doesn't care about race_timeout_ms, apply default
        match input.race_timeout_ms {
            None | Some(0) => {
                input.race_timeout_ms = Some(DEFAULT_RPC_MULTI_RACE_TIMEOUT_MS);
            }
            _ => (),
        }

        // race timeout > timeout is nonesense
        if input.as_race {
            if input.race_timeout_ms.unwrap() > input.timeout_ms.unwrap() {
                input.race_timeout_ms = Some(input.timeout_ms.unwrap());
            }
        }

        self.handle_rpc_multi_inner(input)
    }

    fn handle_notify_multi(
        &mut self,
        mut input: actor::NotifyMulti,
    ) -> KitsuneP2pHandlerResult<u8> {
        // if the user doesn't care about remote_agent_count, apply default
        match input.remote_agent_count {
            None | Some(0) => {
                input.remote_agent_count = Some(DEFAULT_NOTIFY_REMOTE_AGENT_COUNT);
            }
            _ => (),
        }

        // if the user doesn't care about timeout_ms, apply default
        // also - if set to 0, we want to return immediately, but
        // spawn a task with that default timeout.
        let do_spawn = match input.timeout_ms {
            None | Some(0) => {
                input.timeout_ms = Some(DEFAULT_NOTIFY_TIMEOUT_MS);
                true
            }
            _ => false,
        };

        // gather the inner future
        let inner_fut = match self.handle_notify_multi_inner(input) {
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
}
