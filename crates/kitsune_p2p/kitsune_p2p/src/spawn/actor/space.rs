use super::*;
use std::collections::HashSet;

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

/// Normally network lookups / connections will be async / take some time.
/// While we are in "short-circuit-only" mode - we just need to allow some
/// time for other agenst to be connected to this conductor.
/// This value does NOT have to be correct, it just has to work.
const NET_CONNECT_INTERVAL_MS: u64 = 20;

/// Max amount of time we should wait for connections to be established.
const NET_CONNECT_MAX_MS: u64 = 2000;

ghost_actor::ghost_chan! {
    pub(crate) chan SpaceInternal<crate::KitsuneP2pError> {
        /// Make a remote request right-now if we have an open connection,
        /// otherwise, return an error.
        fn immediate_request(space: Arc<KitsuneSpace>, agent: Arc<KitsuneAgent>, data: Arc<Vec<u8>>) -> Vec<u8>;

        /// List online agents that claim to be covering a basis hash
        fn list_online_agents_for_basis_hash(space: Arc<KitsuneSpace>, basis: Arc<KitsuneBasis>) -> Vec<Arc<KitsuneAgent>>;
    }
}

pub(crate) async fn spawn_space(
    space: Arc<KitsuneSpace>,
) -> KitsuneP2pResult<(
    ghost_actor::GhostSender<KitsuneP2p>,
    KitsuneP2pEventReceiver,
)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let internal_sender = builder
        .channel_factory()
        .create_channel::<SpaceInternal>()
        .await?;

    let sender = builder
        .channel_factory()
        .create_channel::<KitsuneP2p>()
        .await?;

    tokio::task::spawn(builder.spawn(Space::new(space, internal_sender, evt_send)));

    Ok((sender, evt_recv))
}

impl ghost_actor::GhostHandler<SpaceInternal> for Space {}

impl SpaceInternalHandler for Space {
    fn handle_immediate_request(
        &mut self,
        _space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        data: Arc<Vec<u8>>,
    ) -> SpaceInternalHandlerResult<Vec<u8>> {
        // Right now we are only implementing the "short-circuit"
        // that routes messages to other agents joined on this same system.
        // I.e. we don't bother with peer discovery because we know the
        // remote is local.
        if !self.agents.contains_key(&agent) {
            return Err(KitsuneP2pError::RoutingAgentError(agent));
        }

        // that agent *is* joined - let's forward the request
        let space = self.space.clone();

        // clone the event sender
        let evt_sender = self.evt_sender.clone();

        // As this is a short-circuit - we need to decode the data inline - here.
        // In the future, we will probably need to branch here, so the real
        // networking can forward the encoded data. Or, split immediate_request
        // into two variants, one for short-circuit, and one for real networking.
        let data = wire::Wire::decode((*data).clone())?;

        match data {
            wire::Wire::Call(payload) => {
                Ok(async move { evt_sender.call(space, agent, payload).await }
                    .boxed()
                    .into())
            }
            wire::Wire::Notify(payload) => {
                Ok(async move {
                    evt_sender.notify(space, agent, payload).await?;
                    // broadcast doesn't return anything...
                    Ok(vec![])
                }
                .boxed()
                .into())
            }
        }
    }

    fn handle_list_online_agents_for_basis_hash(
        &mut self,
        _space: Arc<KitsuneSpace>,
        // during short-circuit / full-sync mode,
        // we're ignoring the basis_hash and just returning everyone.
        _basis: Arc<KitsuneBasis>,
    ) -> SpaceInternalHandlerResult<Vec<Arc<KitsuneAgent>>> {
        let res = self.agents.keys().cloned().collect();
        Ok(async move { Ok(res) }.boxed().into())
    }
}

impl ghost_actor::GhostControlHandler for Space {}

impl ghost_actor::GhostHandler<KitsuneP2p> for Space {}

impl KitsuneP2pHandler for Space {
    fn handle_join(
        &mut self,
        _space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        match self.agents.entry(agent.clone()) {
            Entry::Occupied(_) => (),
            Entry::Vacant(entry) => {
                entry.insert(AgentInfo { agent });
            }
        }
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_leave(
        &mut self,
        _space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        self.agents.remove(&agent);
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_rpc_single(
        &mut self,
        _space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space = self.space.clone();
        let internal_sender = self.internal_sender.clone();
        let payload = Arc::new(wire::Wire::call(payload).encode());

        Ok(async move {
            let start = std::time::Instant::now();

            loop {
                // attempt to send the request right now
                let err = match internal_sender
                    .immediate_request(space.clone(), agent.clone(), payload.clone())
                    .await
                {
                    Ok(res) => return Ok(res),
                    Err(e) => Err(e),
                };

                // the attempt failed
                // see if we have been trying too long
                if start.elapsed().as_millis() as u64 > NET_CONNECT_MAX_MS {
                    return err;
                }

                // the attempt failed - wait a bit to allow agents to connect
                tokio::time::delay_for(std::time::Duration::from_millis(NET_CONNECT_INTERVAL_MS))
                    .await;
            }
        }
        .boxed()
        .into())
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

/// Local helper struct for associating info with a connected agent.
struct AgentInfo {
    #[allow(dead_code)]
    agent: Arc<KitsuneAgent>,
}

/// A Kitsune P2p Node can track multiple "spaces" -- Non-interacting namespaced
/// areas that share common transport infrastructure for communication.
pub(crate) struct Space {
    space: Arc<KitsuneSpace>,
    internal_sender: ghost_actor::GhostSender<SpaceInternal>,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    agents: HashMap<Arc<KitsuneAgent>, AgentInfo>,
}

impl Space {
    /// space constructor
    pub fn new(
        space: Arc<KitsuneSpace>,
        internal_sender: ghost_actor::GhostSender<SpaceInternal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) -> Self {
        Self {
            space,
            internal_sender,
            evt_sender,
            agents: HashMap::new(),
        }
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
                        if !sent_to.contains(&agent) {
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
