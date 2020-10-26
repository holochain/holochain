use super::*;
use ghost_actor::dependencies::{tracing, tracing_futures::Instrument};
use kitsune_p2p_types::codec::Codec;
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
        fn immediate_request(space: Arc<KitsuneSpace>, to_agent: Arc<KitsuneAgent>, from_agent: Arc<KitsuneAgent>, data: Arc<Vec<u8>>) -> Vec<u8>;

        /// List online agents that claim to be covering a basis hash
        fn list_online_agents_for_basis_hash(space: Arc<KitsuneSpace>, basis: Arc<KitsuneBasis>) -> Vec<Arc<KitsuneAgent>>;
    }
}

pub(crate) async fn spawn_space(
    space: Arc<KitsuneSpace>,
    transport: ghost_actor::GhostSender<TransportListener>,
) -> KitsuneP2pResult<(
    ghost_actor::GhostSender<KitsuneP2p>,
    KitsuneP2pEventReceiver,
)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    // initialize gossip module
    let gossip_recv = gossip::spawn_gossip_module();
    builder
        .channel_factory()
        .attach_receiver(gossip_recv)
        .await?;

    let internal_sender = builder
        .channel_factory()
        .create_channel::<SpaceInternal>()
        .await?;

    let sender = builder
        .channel_factory()
        .create_channel::<KitsuneP2p>()
        .await?;

    tokio::task::spawn(builder.spawn(Space::new(space, internal_sender, evt_send, transport)));

    Ok((sender, evt_recv))
}

impl ghost_actor::GhostHandler<gossip::GossipEvent> for Space {}

impl gossip::GossipEventHandler for Space {
    fn handle_list_neighbor_agents(
        &mut self,
    ) -> gossip::GossipEventHandlerResult<Vec<Arc<KitsuneAgent>>> {
        // while full-sync this is just a clone of list_by_basis
        let res = self.agents.keys().cloned().collect();
        Ok(async move { Ok(res) }.boxed().into())
    }

    fn handle_req_op_hashes(
        &mut self,
        _from_agent: Arc<KitsuneAgent>,
        to_agent: Arc<KitsuneAgent>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
        since_utc_epoch_s: i64,
        until_utc_epoch_s: i64,
    ) -> gossip::GossipEventHandlerResult<Vec<Arc<KitsuneOpHash>>> {
        // while full-sync just redirecting to self...
        // but eventually some of these will be outgoing remote requests
        let fut = self
            .evt_sender
            .fetch_op_hashes_for_constraints(FetchOpHashesForConstraintsEvt {
                space: self.space.clone(),
                agent: to_agent,
                dht_arc,
                since_utc_epoch_s,
                until_utc_epoch_s,
            });
        Ok(async move { fut.await }.boxed().into())
    }

    fn handle_req_op_data(
        &mut self,
        _from_agent: Arc<KitsuneAgent>,
        to_agent: Arc<KitsuneAgent>,
        op_hashes: Vec<Arc<KitsuneOpHash>>,
    ) -> gossip::GossipEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
        // while full-sync just redirecting to self...
        // but eventually some of these will be outgoing remote requests
        let fut = self.evt_sender.fetch_op_hash_data(FetchOpHashDataEvt {
            space: self.space.clone(),
            agent: to_agent,
            op_hashes,
        });
        Ok(async move { fut.await }.boxed().into())
    }

    fn handle_gossip_ops(
        &mut self,
        from_agent: Arc<KitsuneAgent>,
        to_agent: Arc<KitsuneAgent>,
        ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
    ) -> gossip::GossipEventHandlerResult<()> {
        let all = ops
            .into_iter()
            .map(|(op_hash, op_data)| {
                self.evt_sender.gossip(
                    self.space.clone(),
                    to_agent.clone(),
                    from_agent.clone(),
                    op_hash,
                    op_data,
                )
            })
            .collect::<Vec<_>>();
        Ok(async move {
            futures::stream::iter(all)
                .for_each_concurrent(10, |res| async move {
                    if let Err(e) = res.await {
                        ghost_actor::dependencies::tracing::error!(?e);
                    }
                })
                .await;
            Ok(())
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<SpaceInternal> for Space {}

impl SpaceInternalHandler for Space {
    fn handle_immediate_request(
        &mut self,
        _space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        data: Arc<Vec<u8>>,
    ) -> SpaceInternalHandlerResult<Vec<u8>> {
        // Right now we are only implementing the "short-circuit"
        // that routes messages to other agents joined on this same system.
        // I.e. we don't bother with peer discovery because we know the
        // remote is local.
        if !self.agents.contains_key(&to_agent) {
            return Err(KitsuneP2pError::RoutingAgentError(to_agent));
        }

        // to_agent *is* joined - let's forward the request
        let space = self.space.clone();

        // clone the event sender
        let evt_sender = self.evt_sender.clone();

        // As this is a short-circuit - we need to decode the data inline - here.
        // In the future, we will probably need to branch here, so the real
        // networking can forward the encoded data. Or, split immediate_request
        // into two variants, one for short-circuit, and one for real networking.
        let (_, data) = wire::Wire::decode_ref(&data)?;

        match data {
            wire::Wire::Call(payload) => Ok(async move {
                evt_sender
                    .call(space, to_agent, from_agent, payload.data.into())
                    .await
            }
            .instrument(tracing::debug_span!("wire_call"))
            .boxed()
            .into()),
            wire::Wire::Notify(payload) => {
                Ok(async move {
                    evt_sender
                        .notify(space, to_agent, from_agent, payload.data.into())
                        .await?;
                    // broadcast doesn't return anything...
                    Ok(vec![])
                }
                .boxed()
                .into())
            }
            _ => {
                tracing::warn!("UNHANDLED WIRE: {:?}", data);
                Ok(async move { Ok(vec![]) }.boxed().into())
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
    fn handle_list_transport_bindings(&mut self) -> KitsuneP2pHandlerResult<Vec<url2::Url2>> {
        unreachable!("These requests are handled at the to actor level and are never propagated down to the space.")
    }

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
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space = self.space.clone();
        let internal_sender = self.internal_sender.clone();
        let payload = Arc::new(wire::Wire::call(payload.into()).encode_vec()?);

        Ok(async move {
            let start = std::time::Instant::now();

            loop {
                // attempt to send the request right now
                let err = match internal_sender
                    .immediate_request(
                        space.clone(),
                        to_agent.clone(),
                        from_agent.clone(),
                        payload.clone(),
                    )
                    .instrument(ghost_actor::dependencies::tracing::debug_span!(
                        "handle_rpc_single_loop"
                    ))
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
        if input.as_race && input.race_timeout_ms.unwrap() > input.timeout_ms.unwrap() {
            input.race_timeout_ms = Some(input.timeout_ms.unwrap());
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
    #[allow(dead_code)]
    transport: ghost_actor::GhostSender<TransportListener>,
    agents: HashMap<Arc<KitsuneAgent>, AgentInfo>,
}

impl Space {
    /// space constructor
    pub fn new(
        space: Arc<KitsuneSpace>,
        internal_sender: ghost_actor::GhostSender<SpaceInternal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
        transport: ghost_actor::GhostSender<TransportListener>,
    ) -> Self {
        Self {
            space,
            internal_sender,
            evt_sender,
            transport,
            agents: HashMap::new(),
        }
    }

    /// actual logic for handle_rpc_multi ...
    /// the top-level handler may or may not spawn a task for this
    #[allow(unused_variables, unused_assignments, unused_mut)]
    #[tracing::instrument(skip(self, input))]
    fn handle_rpc_multi_inner(
        &mut self,
        input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        let actor::RpcMulti {
            space,
            from_agent,
            basis,
            //remote_agent_count,
            //timeout_ms,
            //as_race,
            //race_timeout_ms,
            payload,
            ..
        } = input;

        // encode the data to send
        let payload = Arc::new(wire::Wire::call(payload.into()).encode_vec()?);

        // TODO - we cannot write proper logic here until we have a
        //        proper peer discovery mechanism. Instead, let's
        //        give it 100 ms max to see if there is any agent
        //        other than us - prefer that, or fall back to
        //        just reflecting the msg to ourselves.

        let i_s = self.internal_sender.clone();
        Ok(async move {
            let mut to_agent = from_agent.clone();
            'search_loop: for _ in 0..5 {
                if let Ok(agent_list) = i_s
                    .list_online_agents_for_basis_hash(space.clone(), basis.clone())
                    .await
                {
                    for a in agent_list {
                        if a != from_agent {
                            to_agent = a;
                            break 'search_loop;
                        }
                    }
                }

                tokio::time::delay_for(std::time::Duration::from_millis(20)).await;
            }

            let mut out = Vec::new();

            // Timeout on immediate requests after a small interval.
            // TODO: 20 ms is only appropriate for local calls and not
            // real networking
            if let Ok(Ok(response)) = tokio::time::timeout(
                std::time::Duration::from_millis(20),
                i_s.immediate_request(space, to_agent.clone(), from_agent.clone(), payload),
            )
            .await
            {
                out.push(actor::RpcMultiResponse {
                    agent: to_agent,
                    response,
                });
            }

            Ok(out)
        }
        .instrument(tracing::debug_span!("multi_inner"))
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
            from_agent,
            basis,
            // ignore remote_agent_count for now - broadcast to everyone
            remote_agent_count: _,
            timeout_ms,
            payload,
        } = input;

        let timeout_ms = timeout_ms.expect("set by handle_notify_multi");

        // encode the data to send
        let payload = Arc::new(wire::Wire::notify(payload.into()).encode_vec()?);

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
                    for to_agent in agent_list {
                        if !sent_to.contains(&to_agent) {
                            sent_to.insert(to_agent.clone());
                            // send the notify here - but spawn
                            // so we're not holding up this loop
                            let internal_sender = internal_sender.clone();
                            let space = space.clone();
                            let payload = payload.clone();
                            let send_success_count = send_success_count.clone();
                            let from_agent2 = from_agent.clone();
                            tokio::task::spawn(
                                async move {
                                    if internal_sender
                                        .immediate_request(space, to_agent, from_agent2, payload)
                                        .await
                                        .is_ok()
                                    {
                                        send_success_count
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                }
                                .instrument(
                                    ghost_actor::dependencies::tracing::debug_span!(
                                        "handle_rpc_multi_inner_loop"
                                    ),
                                ),
                            );
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
