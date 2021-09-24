use super::*;
use crate::types::gossip::GossipModule;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_mdns::*;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::codec::{rmp_decode, rmp_encode};
use kitsune_p2p_types::dht_arc::DhtArc;
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use url2::Url2;

mod rpc_multi_logic;

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KBasis = Arc<KitsuneBasis>;
type WireConHnd = Tx2ConHnd<wire::Wire>;
type Payload = Box<[u8]>;

ghost_actor::ghost_chan! {
    pub(crate) chan SpaceInternal<crate::KitsuneP2pError> {
        /// List online agents that claim to be covering a basis hash
        fn list_online_agents_for_basis_hash(space: KSpace, from_agent: KAgent, basis: KBasis) -> HashSet<KAgent>;

        /// Update / publish our agent info
        fn update_agent_info() -> ();

        /// Update / publish a single agent info
        fn update_single_agent_info(agent: KAgent) -> ();

        /// see if an agent is locally joined
        fn is_agent_local(agent: KAgent) -> bool;

        /// Update the arc of a local agent.
        fn update_agent_arc(agent: KAgent, arc: DhtArc) -> ();

        /// Incoming Delegate Broadcast
        /// We are being requested to delegate a broadcast to our neighborhood
        /// on behalf of an author. `mod_idx` / `mod_cnt` inform us which
        /// neighbors we are responsible for.
        /// (See comments in actual method impl for more detail.)
        fn incoming_delegate_broadcast(
            space: KSpace,
            basis: KBasis,
            to_agent: KAgent,
            mod_idx: u32,
            mod_cnt: u32,
            data: crate::wire::WireData,
        ) -> ();

        /// Incoming Gossip
        fn incoming_gossip(space: KSpace, con: WireConHnd, remote_url: TxUrl, data: Payload, module_type: crate::types::gossip::GossipModuleType) -> ();
    }
}

pub(crate) async fn spawn_space(
    space: Arc<KitsuneSpace>,
    this_addr: url2::Url2,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    config: Arc<KitsuneP2pConfig>,
    bandwidth_throttles: BandwidthThrottles,
    parallel_notify_permit: Arc<tokio::sync::Semaphore>,
) -> KitsuneP2pResult<(
    ghost_actor::GhostSender<KitsuneP2p>,
    ghost_actor::GhostSender<SpaceInternal>,
    KitsuneP2pEventReceiver,
)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let i_s = builder
        .channel_factory()
        .create_channel::<SpaceInternal>()
        .await?;

    let sender = builder
        .channel_factory()
        .create_channel::<KitsuneP2p>()
        .await?;

    tokio::task::spawn(builder.spawn(Space::new(
        space,
        this_addr,
        i_s.clone(),
        evt_send,
        ep_hnd,
        config,
        bandwidth_throttles,
        parallel_notify_permit,
    )));

    Ok((sender, i_s, evt_recv))
}

impl ghost_actor::GhostHandler<SpaceInternal> for Space {}

impl SpaceInternalHandler for Space {
    fn handle_list_online_agents_for_basis_hash(
        &mut self,
        _space: Arc<KitsuneSpace>,
        _from_agent: Arc<KitsuneAgent>,
        // during short-circuit / full-sync mode,
        // we're ignoring the basis_hash and just returning everyone.
        _basis: Arc<KitsuneBasis>,
    ) -> SpaceInternalHandlerResult<HashSet<Arc<KitsuneAgent>>> {
        let mut res: HashSet<Arc<KitsuneAgent>> =
            self.local_joined_agents.iter().cloned().collect();
        let all_peers_fut = self
            .evt_sender
            .query_agent_info_signed(QueryAgentInfoSignedEvt {
                space: self.space.clone(),
                agents: None,
            });
        Ok(async move {
            for peer in all_peers_fut.await? {
                res.insert(peer.agent.clone());
            }
            Ok(res)
        }
        .boxed()
        .into())
    }

    fn handle_update_agent_info(&mut self) -> SpaceInternalHandlerResult<()> {
        let space = self.space.clone();
        let mut mdns_handles = self.mdns_handles.clone();
        let network_type = self.config.network_type.clone();
        let mut agent_list = Vec::with_capacity(self.local_joined_agents.len());
        for agent in self.local_joined_agents.iter().cloned() {
            let arc = self.get_agent_arc(&agent);
            agent_list.push((agent, arc));
        }
        let bound_url = self.this_addr.clone();
        let evt_sender = self.evt_sender.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        let expires_after = self.config.tuning_params.agent_info_expires_after_ms as u64;
        let dynamic_arcs = self.config.tuning_params.gossip_dynamic_arcs;
        let single_storage_arc_per_space = self
            .config
            .tuning_params
            .gossip_single_storage_arc_per_space;
        let internal_sender = self.i_s.clone();
        Ok(async move {
            let urls = vec![bound_url.into()];
            let mut peer_data = Vec::with_capacity(agent_list.len());
            for (agent, arc) in agent_list {
                let input = UpdateAgentInfoInput {
                    expires_after,
                    space: space.clone(),
                    agent,
                    arc,
                    urls: &urls,
                    evt_sender: &evt_sender,
                    internal_sender: &internal_sender,
                    network_type: network_type.clone(),
                    mdns_handles: &mut mdns_handles,
                    bootstrap_service: &bootstrap_service,
                    dynamic_arcs,
                    single_storage_arc_per_space,
                };
                peer_data.push(update_single_agent_info(input).await?);
            }
            evt_sender
                .put_agent_info_signed(PutAgentInfoSignedEvt {
                    space: space.clone(),
                    peer_data,
                })
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_update_single_agent_info(
        &mut self,
        agent: Arc<KitsuneAgent>,
    ) -> SpaceInternalHandlerResult<()> {
        let space = self.space.clone();
        let mut mdns_handles = self.mdns_handles.clone();
        let network_type = self.config.network_type.clone();
        let bound_url = self.this_addr.clone();
        let evt_sender = self.evt_sender.clone();
        let internal_sender = self.i_s.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        let expires_after = self.config.tuning_params.agent_info_expires_after_ms as u64;
        let dynamic_arcs = self.config.tuning_params.gossip_dynamic_arcs;
        let single_storage_arc_per_space = self
            .config
            .tuning_params
            .gossip_single_storage_arc_per_space;
        let arc = self.get_agent_arc(&agent);

        Ok(async move {
            let urls = vec![bound_url.into()];
            let input = UpdateAgentInfoInput {
                expires_after,
                space: space.clone(),
                agent,
                arc,
                urls: &urls,
                evt_sender: &evt_sender,
                internal_sender: &internal_sender,
                network_type: network_type.clone(),
                mdns_handles: &mut mdns_handles,
                bootstrap_service: &bootstrap_service,
                dynamic_arcs,
                single_storage_arc_per_space,
            };
            let peer_data = vec![update_single_agent_info(input).await?];
            evt_sender
                .put_agent_info_signed(PutAgentInfoSignedEvt {
                    space: space.clone(),
                    peer_data,
                })
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_is_agent_local(
        &mut self,
        agent: Arc<KitsuneAgent>,
    ) -> SpaceInternalHandlerResult<bool> {
        let res = self.local_joined_agents.contains(&agent);
        Ok(async move { Ok(res) }.boxed().into())
    }

    fn handle_update_agent_arc(
        &mut self,
        agent: Arc<KitsuneAgent>,
        arc: DhtArc,
    ) -> SpaceInternalHandlerResult<()> {
        self.agent_arcs.insert(agent, arc);
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_incoming_delegate_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        basis: Arc<KitsuneBasis>,
        _to_agent: Arc<KitsuneAgent>,
        mod_idx: u32,
        mod_cnt: u32,
        data: crate::wire::WireData,
    ) -> InternalHandlerResult<()> {
        // first, forward this incoming broadcast to all connected
        // local agents.
        let mut local_events = Vec::new();
        for agent in self.local_joined_agents.iter().cloned() {
            if let Some(arc) = self.agent_arcs.get(&agent) {
                if arc.contains(basis.get_loc()) {
                    let fut = self.evt_sender.notify(
                        space.clone(),
                        agent.clone(),
                        agent.clone(),
                        data.clone().into(),
                    );
                    local_events.push(async move {
                        if let Err(err) = fut.await {
                            tracing::warn!(?err, "failed local broadcast");
                        }
                    });
                }
            }
        }

        // next, gather a list of agents covering this data to be
        // published to.
        let ro_inner = self.ro_inner.clone();
        let timeout = ro_inner.config.tuning_params.implicit_timeout();
        let fut =
            discover::get_cached_remotes_near_basis(ro_inner.clone(), basis.get_loc(), timeout);

        Ok(async move {
            futures::future::join_all(local_events).await;

            let info_list = fut.await?;

            // for all agents in the gathered list, check the modulo params
            // i.e. if `agent.get_loc() % mod_cnt == mod_idx` we know we are
            // responsible for delegating the broadcast to that agent.
            let mut all = Vec::new();
            for info in info_list
                .into_iter()
                .filter(|info| info.agent.get_loc() % mod_cnt == mod_idx)
            {
                let ro_inner = ro_inner.clone();
                let space = space.clone();
                let data = data.clone();
                all.push(async move {
                    use discover::PeerDiscoverResult;

                    // attempt to establish a connection
                    let con_hnd = match discover::peer_connect(ro_inner, &info, timeout).await {
                        PeerDiscoverResult::OkShortcut => return,
                        PeerDiscoverResult::OkRemote { con_hnd, .. } => con_hnd,
                        PeerDiscoverResult::Err(err) => {
                            tracing::warn!(?err, "broadcast error");
                            return;
                        }
                    };

                    // generate our broadcast payload
                    let payload = wire::Wire::broadcast(space, info.agent.clone(), data);

                    // forward the data
                    if let Err(err) = con_hnd.notify(&payload, timeout).await {
                        tracing::warn!(?err, "broadcast error");
                    }
                })
            }

            futures::future::join_all(all).await;

            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_incoming_gossip(
        &mut self,
        _space: Arc<KitsuneSpace>,
        con: Tx2ConHnd<wire::Wire>,
        remote_url: TxUrl,
        data: Box<[u8]>,
        module_type: GossipModuleType,
    ) -> InternalHandlerResult<()> {
        match self.gossip_mod.get(&module_type) {
            Some(module) => module.incoming_gossip(con, remote_url, data)?,
            None => tracing::warn!(
                "Received gossip for {:?} but this gossip module isn't running",
                module_type
            ),
        }
        unit_ok_fut()
    }
}

struct UpdateAgentInfoInput<'borrow> {
    expires_after: u64,
    space: Arc<KitsuneSpace>,
    agent: Arc<KitsuneAgent>,
    arc: DhtArc,
    urls: &'borrow Vec<TxUrl>,
    evt_sender: &'borrow futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    internal_sender: &'borrow ghost_actor::GhostSender<SpaceInternal>,
    network_type: NetworkType,
    mdns_handles: &'borrow mut HashMap<Vec<u8>, Arc<AtomicBool>>,
    bootstrap_service: &'borrow Option<Url2>,
    dynamic_arcs: bool,
    single_storage_arc_per_space: bool,
}

async fn update_arc_length(
    evt_sender: &futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    space: Arc<KitsuneSpace>,
    arc: &mut DhtArc,
) -> KitsuneP2pResult<()> {
    let density = evt_sender.query_peer_density(space.clone(), *arc).await?;
    arc.update_length(density);
    Ok(())
}

async fn update_single_agent_info(
    input: UpdateAgentInfoInput<'_>,
) -> KitsuneP2pResult<AgentInfoSigned> {
    let UpdateAgentInfoInput {
        expires_after,
        space,
        agent,
        mut arc,
        urls,
        evt_sender,
        internal_sender,
        network_type,
        mdns_handles,
        bootstrap_service,
        dynamic_arcs,
        single_storage_arc_per_space,
    } = input;

    // If there is only a single agent per space don't update the empty arcs.
    let should_not_update_arc_length = single_storage_arc_per_space && arc.is_empty();

    if dynamic_arcs && !should_not_update_arc_length {
        update_arc_length(evt_sender, space.clone(), &mut arc).await?;
    }

    // Update the agents arc through the internal sender.
    internal_sender.update_agent_arc(agent.clone(), arc).await?;

    let signed_at_ms = crate::spawn::actor::bootstrap::now_once(None).await?;
    let expires_at_ms = signed_at_ms + expires_after;

    let agent_info_signed = AgentInfoSigned::sign(
        space.clone(),
        agent.clone(),
        arc.half_length(),
        urls.clone(),
        signed_at_ms,
        expires_at_ms,
        |d| {
            let data = Arc::new(d.to_vec());
            async {
                let sign_req = SignNetworkDataEvt {
                    space: space.clone(),
                    agent: agent.clone(),
                    data,
                };
                evt_sender
                    .sign_network_data(sign_req)
                    .await
                    .map(Arc::new)
                    .map_err(KitsuneError::other)
            }
        },
    )
    .await?;

    tracing::debug!(?agent_info_signed);

    // Push to the network as well
    match network_type {
        NetworkType::QuicMdns => {
            // Broadcast only valid AgentInfo
            if !urls.is_empty() {
                // Kill previous broadcast for this space + agent
                let key = [space.get_bytes(), agent.get_bytes()].concat();
                if let Some(current_handle) = mdns_handles.get(&key) {
                    mdns_kill_thread(current_handle.to_owned());
                }
                // Broadcast by using Space as service type and Agent as service name
                let space_b64 = base64::encode_config(&space[..], base64::URL_SAFE_NO_PAD);
                let agent_b64 = base64::encode_config(&agent[..], base64::URL_SAFE_NO_PAD);
                //println!("(MDNS) - Broadcasting of Agent {:?} ({}) in space {:?} ({} ; {})",
                // agent, agent.get_bytes().len(), space, space.get_bytes().len(), space_b64.len());
                // Broadcast rmp encoded agent_info_signed
                let mut buffer = Vec::new();
                rmp_encode(&mut buffer, &agent_info_signed)?;
                tracing::trace!(?space_b64, ?agent_b64);
                let handle = mdns_create_broadcast_thread(space_b64, agent_b64, &buffer);
                // store handle in self
                mdns_handles.insert(key, handle);
            }
        }
        NetworkType::QuicBootstrap => {
            crate::spawn::actor::bootstrap::put(
                bootstrap_service.clone(),
                agent_info_signed.clone(),
            )
            .await?;
        }
    }
    Ok(agent_info_signed)
}

use ghost_actor::dependencies::must_future::MustBoxFuture;
impl ghost_actor::GhostControlHandler for Space {
    fn handle_ghost_actor_shutdown(mut self) -> MustBoxFuture<'static, ()> {
        async move {
            use futures::sink::SinkExt;
            // this is a curtesy, ok if fails
            let _ = self.evt_sender.close().await;
            for module in self.gossip_mod.values_mut() {
                module.close();
            }
        }
        .boxed()
        .into()
    }
}

impl ghost_actor::GhostHandler<KitsuneP2p> for Space {}

impl KitsuneP2pHandler for Space {
    fn handle_list_transport_bindings(&mut self) -> KitsuneP2pHandlerResult<Vec<url2::Url2>> {
        unreachable!(
            "These requests are handled at the to actor level and are never propagated down to the space."
        )
    }

    fn handle_join(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        self.local_joined_agents.insert(agent.clone());
        for module in self.gossip_mod.values() {
            module.local_agent_join(agent.clone());
        }
        let fut = self.i_s.update_single_agent_info(agent);
        let evt_sender = self.evt_sender.clone();
        match self.config.network_type {
            NetworkType::QuicMdns => {
                // Listen to MDNS service that has that space as service type
                let space_b64 = base64::encode_config(&space[..], base64::URL_SAFE_NO_PAD);
                if !self.mdns_listened_spaces.contains(&space_b64) {
                    self.mdns_listened_spaces.insert(space_b64.clone());
                    tokio::task::spawn(async move {
                        let stream = mdns_listen(space_b64);
                        tokio::pin!(stream);
                        while let Some(maybe_response) = stream.next().await {
                            match maybe_response {
                                Ok(response) => {
                                    tracing::trace!(msg = "Peer found via MDNS", ?response);
                                    // Decode response
                                    let maybe_agent_info_signed =
                                        rmp_decode(&mut &*response.buffer);
                                    if let Err(e) = maybe_agent_info_signed {
                                        tracing::error!(msg = "Failed to decode MDNS peer", ?e);
                                        continue;
                                    }
                                    if let Ok(remote_agent_info_signed) = maybe_agent_info_signed {
                                        // Add to local storage
                                        if let Err(e) = evt_sender
                                            .put_agent_info_signed(PutAgentInfoSignedEvt {
                                                space: space.clone(),
                                                peer_data: vec![remote_agent_info_signed],
                                            })
                                            .await
                                        {
                                            tracing::error!(msg = "Failed to store MDNS peer", ?e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(msg = "Failed to get peers from MDNS", ?e);
                                }
                            }
                        }
                    });
                }
            }
            NetworkType::QuicBootstrap => {
                // quic bootstrap is managed for the whole space
                // see the Space::new() constructor
            }
        }

        Ok(async move { fut.await }.boxed().into())
    }

    fn handle_leave(
        &mut self,
        _space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        self.local_joined_agents.remove(&agent);
        self.agent_arcs.remove(&agent);
        for module in self.gossip_mod.values() {
            module.local_agent_leave(agent.clone());
        }
        self.publish_leave_agent_info(agent)
    }

    fn handle_rpc_single(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
        timeout_ms: Option<u64>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();

        let timeout_ms = match timeout_ms {
            None | Some(0) => self.config.tuning_params.default_rpc_single_timeout_ms as u64,
            _ => timeout_ms.unwrap(),
        };
        let timeout = KitsuneTimeout::from_millis(timeout_ms);

        let discover_fut = discover::search_and_discover_peer_connect(
            self.ro_inner.clone(),
            to_agent.clone(),
            timeout,
        );

        Ok(async move {
            match discover_fut.await {
                discover::PeerDiscoverResult::OkShortcut => {
                    // reflect this request locally
                    evt_sender.call(space, to_agent, from_agent, payload).await
                }
                discover::PeerDiscoverResult::OkRemote { con_hnd, .. } => {
                    let payload = wire::Wire::call(
                        space.clone(),
                        from_agent.clone(),
                        to_agent.clone(),
                        payload.into(),
                    );
                    let res = con_hnd.request(&payload, timeout).await?;
                    match res {
                        wire::Wire::Failure(wire::Failure { reason }) => Err(reason.into()),
                        wire::Wire::CallResp(wire::CallResp { data }) => Ok(data.into()),
                        r => Err(format!("invalid response: {:?}", r).into()),
                    }
                }
                discover::PeerDiscoverResult::Err(e) => Err(e),
            }
        }
        .boxed()
        .into())
    }

    fn handle_rpc_multi(
        &mut self,
        input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        let fut = rpc_multi_logic::handle_rpc_multi(
            input,
            self.ro_inner.clone(),
            self.local_joined_agents.clone(),
        );
        Ok(async move { fut.await }.boxed().into())
    }

    fn handle_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        basis: Arc<KitsuneBasis>,
        timeout: KitsuneTimeout,
        payload: Vec<u8>,
    ) -> KitsuneP2pHandlerResult<()> {
        // first, forward this data to all connected local agents.
        let mut local_events = Vec::new();
        for agent in self.local_joined_agents.iter().cloned() {
            if let Some(arc) = self.agent_arcs.get(&agent) {
                if arc.contains(basis.get_loc()) {
                    let fut = self.evt_sender.notify(
                        space.clone(),
                        agent.clone(),
                        agent.clone(),
                        payload.clone(),
                    );
                    local_events.push(async move {
                        if let Err(err) = fut.await {
                            tracing::warn!(?err, "failed local broadcast");
                        }
                    });
                }
            }
        }

        // then, find a list of agents in a potentially remote neighborhood
        // that should be responsible for holding the data.
        let ro_inner = self.ro_inner.clone();
        let discover_fut =
            discover::search_remotes_covering_basis(ro_inner.clone(), basis.get_loc(), timeout);
        Ok(async move {
            futures::future::join_all(local_events).await;

            // TODO - FIXME
            // Holochain currently does all its testing without any remote nodes
            // if we do this inline, it takes us to the 30 second timeout
            // on every one of those... so spawning for now, which means
            // we won't get notified if we are unable to publish to anyone.
            // Also, if conductor spams us with publishes, we could fill
            // the memory with publish tasks.
            let task_permit = ro_inner
                .parallel_notify_permit
                .clone()
                .acquire_owned()
                .await
                .ok();
            tokio::task::spawn(async move {
                let cover_nodes = discover_fut.await?;
                if cover_nodes.is_empty() {
                    return Err("failed to discover neighboring peers".into());
                }

                let mut all = Vec::new();

                // is there a better way to do this??
                //
                // since we're gathering the connections in one place,
                // if any of them take the full timeout, we won't have any
                // time to actually forward the message to them.
                //
                // and if a node is that slow anyways, maybe we don't want
                // to trust them to forward the message in any case...
                let half_timeout =
                    KitsuneTimeout::from_millis(timeout.time_remaining().as_millis() as u64 / 2);

                // attempt to open connections to the discovered remote nodes
                for info in cover_nodes {
                    let ro_inner = ro_inner.clone();
                    all.push(async move {
                        use discover::PeerDiscoverResult;
                        let con_hnd =
                            match discover::peer_connect(ro_inner, &info, half_timeout).await {
                                PeerDiscoverResult::OkShortcut => return None,
                                PeerDiscoverResult::OkRemote { con_hnd, .. } => con_hnd,
                                PeerDiscoverResult::Err(_) => return None,
                            };
                        Some((info.agent.clone(), con_hnd))
                    });
                }

                let con_list = futures::future::join_all(all)
                    .await
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();

                if con_list.is_empty() {
                    return Err("failed to connect to neighboring peers".into());
                }

                let mut all = Vec::new();

                // determine the total number of nodes we'll be publishing to
                // we'll make each remote responsible for a subset of delegate
                // broadcasting by having them apply the formula:
                // `agent.get_loc() % mod_cnt == mod_idx` -- if true,
                // they'll be responsible for forwarding the data to that node.
                let mod_cnt = con_list.len();
                for (mod_idx, (agent, con_hnd)) in con_list.into_iter().enumerate() {
                    // build our delegate message
                    let payload = wire::Wire::delegate_broadcast(
                        space.clone(),
                        basis.clone(),
                        agent,
                        mod_idx as u32,
                        mod_cnt as u32,
                        payload.clone().into(),
                    );

                    // notify the remote node
                    all.push(async move {
                        if let Err(err) = con_hnd.notify(&payload, timeout).await {
                            tracing::warn!(?err, "delegate broadcast error");
                        }
                    });
                }

                futures::future::join_all(all).await;

                drop(task_permit);
                KitsuneP2pResult::Ok(())
            });

            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_targeted_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        from_agent: Arc<KitsuneAgent>,
        agents: Vec<Arc<KitsuneAgent>>,
        timeout: KitsuneTimeout,
        payload: Vec<u8>,
    ) -> KitsuneP2pHandlerResult<()> {
        let evt_sender = self.evt_sender.clone();
        let ro_inner = self.ro_inner.clone();
        let concurrent_limit_per_thread = self.config.tuning_params.concurrent_limit_per_thread;
        Ok(async move {
            // limit spawns with semaphore.
            let task_permit = ro_inner
                .parallel_notify_permit
                .clone()
                .acquire_owned()
                .await
                .ok();
            tokio::task::spawn(async move {
                let mut futures = Vec::with_capacity(agents.len());
                for agent in agents {
                    let discover_fut = async {
                        let result = discover::search_and_discover_peer_connect(
                            ro_inner.clone(),
                            agent.clone(),
                            timeout,
                        )
                        .await;
                        (result, agent)
                    };
                    futures.push(discover_fut);
                }
                let futures = futures::stream::iter(futures);
                let futures = futures.buffer_unordered(concurrent_limit_per_thread);
                futures
                    .for_each_concurrent(concurrent_limit_per_thread, |(discover_result, agent)| {
                        match discover_result {
                            discover::PeerDiscoverResult::OkShortcut => {
                                // reflect this request locally
                                evt_sender
                                    .notify(
                                        space.clone(),
                                        agent,
                                        from_agent.clone(),
                                        payload.clone(),
                                    )
                                    .map(|r| {
                                        if let Err(e) = r {
                                            tracing::error!(
                                                "Failed to broadcast to local agent because: {:?}",
                                                e
                                            )
                                        }
                                    })
                                    .boxed()
                            }
                            discover::PeerDiscoverResult::OkRemote { con_hnd, .. } => {
                                let payload = wire::Wire::broadcast(
                                    space.clone(),
                                    agent,
                                    payload.clone().into(),
                                );
                                con_hnd
                                    .notify(&payload, timeout)
                                    .map(|r| {
                                        if let Err(e) = r {
                                            tracing::info!(
                                                "Failed to broadcast to remote agent because: {:?}",
                                                e
                                            )
                                        }
                                    })
                                    .boxed()
                            }
                            discover::PeerDiscoverResult::Err(e) => async move {
                                tracing::info!(
                                    "Failed to discover connection for {:?} because: {:?}",
                                    agent,
                                    e
                                );
                            }
                            .boxed(),
                        }
                    })
                    .await;
                drop(task_permit)
            });
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_new_integrated_data(&mut self, _: KSpace) -> InternalHandlerResult<()> {
        for module in self.gossip_mod.values() {
            module.new_integrated_data();
        }
        unit_ok_fut()
    }

    fn handle_authority_for_hash(
        &mut self,
        _space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        basis: Arc<KitsuneBasis>,
    ) -> KitsuneP2pHandlerResult<bool> {
        let r = match self.agent_arcs.get(&agent) {
            Some(agent_arc) => agent_arc.contains(basis.get_loc()),
            None => false,
        };
        Ok(async move { Ok(r) }.boxed().into())
    }
}

pub(crate) struct SpaceReadOnlyInner {
    pub(crate) space: Arc<KitsuneSpace>,
    #[allow(dead_code)]
    pub(crate) this_addr: url2::Url2,
    pub(crate) i_s: ghost_actor::GhostSender<SpaceInternal>,
    pub(crate) evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    pub(crate) ep_hnd: Tx2EpHnd<wire::Wire>,
    #[allow(dead_code)]
    pub(crate) config: Arc<KitsuneP2pConfig>,
    pub(crate) parallel_notify_permit: Arc<tokio::sync::Semaphore>,
}

/// A Kitsune P2p Node can track multiple "spaces" -- Non-interacting namespaced
/// areas that share common transport infrastructure for communication.
pub(crate) struct Space {
    pub(crate) ro_inner: Arc<SpaceReadOnlyInner>,
    pub(crate) space: Arc<KitsuneSpace>,
    pub(crate) this_addr: url2::Url2,
    pub(crate) i_s: ghost_actor::GhostSender<SpaceInternal>,
    pub(crate) evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    pub(crate) local_joined_agents: HashSet<Arc<KitsuneAgent>>,
    pub(crate) agent_arcs: HashMap<Arc<KitsuneAgent>, DhtArc>,
    pub(crate) config: Arc<KitsuneP2pConfig>,
    mdns_handles: HashMap<Vec<u8>, Arc<AtomicBool>>,
    mdns_listened_spaces: HashSet<String>,
    gossip_mod: HashMap<GossipModuleType, GossipModule>,
}

impl Space {
    /// space constructor
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        space: Arc<KitsuneSpace>,
        this_addr: url2::Url2,
        i_s: ghost_actor::GhostSender<SpaceInternal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        config: Arc<KitsuneP2pConfig>,
        bandwidth_throttles: BandwidthThrottles,
        parallel_notify_permit: Arc<tokio::sync::Semaphore>,
    ) -> Self {
        let gossip_mod = config
            .tuning_params
            .gossip_strategy
            .split(',')
            .flat_map(|module| match module {
                "simple-bloom" => vec![(
                    GossipModuleType::Simple,
                    crate::gossip::simple_bloom::factory(),
                )],
                "sharded-gossip" => vec![
                    (
                        GossipModuleType::ShardedRecent,
                        crate::gossip::sharded_gossip::recent_factory(bandwidth_throttles.recent()),
                    ),
                    (
                        GossipModuleType::ShardedHistorical,
                        crate::gossip::sharded_gossip::historical_factory(
                            bandwidth_throttles.historical(),
                        ),
                    ),
                ],
                "none" => vec![],
                _ => {
                    panic!("unknown gossip strategy: {}", module);
                }
            })
            .map(|(module, factory)| {
                (
                    module,
                    factory.spawn_gossip_task(
                        config.tuning_params.clone(),
                        space.clone(),
                        ep_hnd.clone(),
                        evt_sender.clone(),
                    ),
                )
            })
            .collect();

        let i_s_c = i_s.clone();
        tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5 * 60)).await;
                if let Err(e) = i_s_c.update_agent_info().await {
                    tracing::error!(failed_to_update_agent_info_for_space = ?e);
                }
            }
        });

        if let NetworkType::QuicBootstrap = &config.network_type {
            // spawn the periodic bootstrap pull
            let i_s_c = i_s.clone();
            let evt_s_c = evt_sender.clone();
            let bootstrap_service = config.bootstrap_service.clone();
            let space_c = space.clone();
            tokio::task::spawn(async move {
                const START_DELAY: std::time::Duration = std::time::Duration::from_secs(1);
                const MAX_DELAY: std::time::Duration = std::time::Duration::from_secs(60 * 60);

                let mut delay_len = START_DELAY;

                loop {
                    use ghost_actor::GhostControlSender;
                    if !i_s_c.ghost_actor_is_active() {
                        break;
                    }

                    tokio::time::sleep(delay_len).await;
                    if delay_len <= MAX_DELAY {
                        delay_len *= 2;
                    }

                    match super::bootstrap::random(
                        bootstrap_service.clone(),
                        kitsune_p2p_types::bootstrap::RandomQuery {
                            space: space_c.clone(),
                            limit: 8.into(),
                        },
                    )
                    .await
                    {
                        Err(e) => {
                            tracing::error!(msg = "Failed to get peers from bootstrap", ?e);
                        }
                        Ok(list) => {
                            if !i_s_c.ghost_actor_is_active() {
                                break;
                            }
                            let mut peer_data = Vec::with_capacity(list.len());
                            for item in list {
                                // TODO - someday some validation here
                                match i_s_c.is_agent_local(item.agent.clone()).await {
                                    Err(err) => tracing::error!(?err),
                                    Ok(is_local) => {
                                        if !is_local {
                                            // we got a result - let's add it to our store for the future
                                            peer_data.push(item);
                                        }
                                    }
                                }
                            }
                            if let Err(err) = evt_s_c
                                .put_agent_info_signed(PutAgentInfoSignedEvt {
                                    space: space_c.clone(),
                                    peer_data,
                                })
                                .await
                            {
                                tracing::error!(?err, "error storing bootstrap agent_info");
                            }
                        }
                    }
                }
                tracing::warn!("bootstrap fetch loop ending");
            });
        }

        let ro_inner = Arc::new(SpaceReadOnlyInner {
            space: space.clone(),
            this_addr: this_addr.clone(),
            i_s: i_s.clone(),
            evt_sender: evt_sender.clone(),
            ep_hnd,
            config: config.clone(),
            parallel_notify_permit,
        });

        Self {
            ro_inner,
            space,
            this_addr,
            i_s,
            evt_sender,
            local_joined_agents: HashSet::new(),
            agent_arcs: HashMap::new(),
            config,
            mdns_handles: HashMap::new(),
            mdns_listened_spaces: HashSet::new(),
            gossip_mod,
        }
    }

    fn publish_leave_agent_info(
        &mut self,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space = self.space.clone();
        let network_type = self.config.network_type.clone();
        let evt_sender = self.evt_sender.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        let expires_after = self.config.tuning_params.agent_info_expires_after_ms as u64;
        Ok(async move {
            let signed_at_ms = crate::spawn::actor::bootstrap::now_once(None).await?;
            let expires_at_ms = signed_at_ms + expires_after;
            let agent_info_signed = AgentInfoSigned::sign(
                space.clone(),
                agent.clone(),
                0,          // no storage arc
                Vec::new(), // no urls
                signed_at_ms,
                expires_at_ms,
                |d| {
                    let data = Arc::new(d.to_vec());
                    async {
                        let sign_req = SignNetworkDataEvt {
                            space: space.clone(),
                            agent: agent.clone(),
                            data,
                        };
                        evt_sender
                            .sign_network_data(sign_req)
                            .await
                            .map(Arc::new)
                            .map_err(KitsuneError::other)
                    }
                },
            )
            .await?;

            tracing::debug!(?agent_info_signed);

            evt_sender
                .put_agent_info_signed(PutAgentInfoSignedEvt {
                    space: space.clone(),
                    peer_data: vec![agent_info_signed.clone()],
                })
                .await?;

            // Push to the network as well
            match network_type {
                NetworkType::QuicMdns => tracing::warn!("NOT publishing leaves to mdns"),
                NetworkType::QuicBootstrap => {
                    crate::spawn::actor::bootstrap::put(
                        bootstrap_service.clone(),
                        agent_info_signed,
                    )
                    .await?;
                }
            }

            Ok(())
        }
        .boxed()
        .into())
    }

    /// Get the existing agent storage arc or create a new one.
    fn get_agent_arc(&self, agent: &Arc<KitsuneAgent>) -> DhtArc {
        if self
            .config
            .tuning_params
            .gossip_single_storage_arc_per_space
        {
            let arc = self.agent_arcs.get(agent).cloned();
            match arc {
                Some(arc) => arc,
                None => {
                    if self.agent_arcs.is_empty() {
                        DhtArc::full(agent.get_loc())
                    } else {
                        DhtArc::empty(agent.get_loc())
                    }
                }
            }
        } else {
            // TODO: We are simply setting the initial arc to full.
            // In the future we may want to do something more intelligent.
            self.agent_arcs
                .get(agent)
                .cloned()
                .unwrap_or_else(|| DhtArc::full(agent.get_loc()))
        }
    }
}
