use super::*;
use crate::metrics::*;
use crate::types::gossip::GossipModule;
use base64::Engine;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_bootstrap_client::BootstrapNet;
use kitsune_p2p_fetch::FetchPool;
use kitsune_p2p_mdns::*;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::codec::{rmp_decode, rmp_encode};
use kitsune_p2p_types::config::KitsuneP2pConfig;
use kitsune_p2p_types::config::NetworkType;
use kitsune_p2p_types::dht::arq::ArqSize;
use kitsune_p2p_types::dht::prelude::ArqClamping;
use kitsune_p2p_types::dht::spacetime::SpaceDimension;
use kitsune_p2p_types::dht::Arq;
use kitsune_p2p_types::dht_arc::{DhtArcRange, DhtArcSet};
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use url2::Url2;

/// How often to record historical metrics
/// (currently once per hour)
const HISTORICAL_METRIC_RECORD_FREQ_MS: u64 = 1000 * 60 * 60;

mod metric_exchange;
use metric_exchange::*;

mod agent_info_update;
mod bootstrap_task;
mod rpc_multi_logic;

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KBasis = Arc<KitsuneBasis>;
type VecMXM = Vec<MetricExchangeMsg>;
pub(crate) type WireConHnd = MetaNetCon;
type Payload = Box<[u8]>;
type OpHashList = Vec<OpHashSized>;
type MaybeDelegate = Option<(KBasis, u32, u32)>;

ghost_actor::ghost_chan! {
    #[allow(clippy::too_many_arguments)]
    pub(crate) chan SpaceInternal<crate::KitsuneP2pError> {
        /// Notification that we have a new address to be identified at
        fn new_address(local_url: String) -> ();

        /// List online agents that claim to be covering a basis hash
        fn list_online_agents_for_basis_hash(space: KSpace, from_agent: KAgent, basis: KBasis) -> HashSet<KAgent>;

        // TODO document these properly so the difference between update single and update is clear

        /// Update / publish our agent info
        fn update_agent_info() -> ();

        /// Update / publish a single agent info
        fn update_single_agent_info(agent: KAgent) -> ();

        // TODO what about this, this is different again and not properly documented
        /// Update / publish a single agent info
        fn publish_agent_info_signed(input: PutAgentInfoSignedEvt) -> ();

        fn get_all_local_joined_agent_infos() -> Vec<AgentInfoSigned>;

        /// see if an agent is locally joined
        fn is_agent_local(agent: KAgent) -> bool;

        /// Update the arc of a local agent.
        fn update_agent_arc(agent: KAgent, arq: Arq) -> ();

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
            data: BroadcastData,
        ) -> ();

        /// This should be invoked instead of incoming_delegate_broadcast
        /// in the case of a publish data variant. It will, in turn, call
        /// into incoming_delegate_broadcast once we have the data to act
        /// as a fetch responder for the op data.
        fn incoming_publish(
            space: KSpace,
            to_agent: KAgent,
            source: KAgent,
            op_hash_list: OpHashList,
            context: kitsune_p2p_fetch::FetchContext,
            maybe_delegate: MaybeDelegate,
        ) -> ();

        /// Send a raw notify.
        fn notify(
            to_agent: KAgent,
            data: wire::Wire,
        ) -> ();

        /// We just received data for an op_hash. Check if we had a pending
        /// delegation action we need to continue now that we have the data.
        fn resolve_publish_pending_delegates(space: KSpace, op_hash: KOpHash) -> ();

        /// Incoming Gossip
        fn incoming_gossip(space: KSpace, con: MetaNetCon, remote_url: String, data: Payload, module_type: crate::types::gossip::GossipModuleType) -> ();

        /// Incoming Metric Exchange
        fn incoming_metric_exchange(space: KSpace, msgs: VecMXM) -> ();

        /// New Con
        fn new_con(url: String, con: WireConHnd) -> ();

        /// Del Con
        fn del_con(url: String) -> ();
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn spawn_space(
    space: Arc<KitsuneSpace>,
    ep_hnd: MetaNet,
    host: HostApi,
    config: Arc<KitsuneP2pConfig>,
    bootstrap_net: BootstrapNet,
    bandwidth_throttles: BandwidthThrottles,
    parallel_notify_permit: Arc<tokio::sync::Semaphore>,
    fetch_pool: FetchPool,
    local_url: Arc<std::sync::Mutex<Option<String>>>,
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

    let host = HostApiLegacy::new(host, evt_send);

    tokio::task::spawn(builder.spawn(Space::new(
        space,
        i_s.clone(),
        host,
        ep_hnd,
        config,
        bootstrap_net,
        bandwidth_throttles,
        parallel_notify_permit,
        fetch_pool,
        local_url,
    )));

    Ok((sender, i_s, evt_recv))
}

impl ghost_actor::GhostHandler<SpaceInternal> for Space {}

impl SpaceInternalHandler for Space {
    fn handle_new_address(&mut self, local_url: String) -> SpaceInternalHandlerResult<()> {
        // shut down open gossip rounds, since we will
        // now be identified differently
        for agent in self.local_joined_agents.keys() {
            for module in self.gossip_mod.values() {
                module.local_agent_leave(agent.clone());
                module.local_agent_join(agent.clone());
            }
        }
        *self.ro_inner.local_url.lock().unwrap() = Some(local_url);
        self.handle_update_agent_info()
    }

    fn handle_list_online_agents_for_basis_hash(
        &mut self,
        _space: Arc<KitsuneSpace>,
        _from_agent: Arc<KitsuneAgent>,
        // during short-circuit / full-sync mode,
        // we're ignoring the basis_hash and just returning everyone.
        _basis: Arc<KitsuneBasis>,
    ) -> SpaceInternalHandlerResult<HashSet<Arc<KitsuneAgent>>> {
        let mut res: HashSet<Arc<KitsuneAgent>> =
            self.local_joined_agents.keys().cloned().collect();
        let all_peers_fut = self
            .host_api
            .legacy
            .query_agents(QueryAgentsEvt::new(self.space.clone()));
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
        let local_url = match self.ro_inner.get_local_url() {
            None => return Ok(async move { Ok(()) }.boxed().into()),
            Some(local_url) => local_url,
        };
        let space = self.space.clone();
        let mut mdns_handles = self.mdns_handles.clone();
        let network_type = self.config.network_type.clone();
        let mut agent_list = Vec::with_capacity(self.local_joined_agents.len());
        for agent in self.local_joined_agents.keys().cloned() {
            let arq = self.get_agent_arq(&agent);
            agent_list.push((agent, arq));
        }
        let bootstrap_net = self.ro_inner.bootstrap_net;
        let evt_sender = self.host_api.legacy.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        let expires_after = self.config.tuning_params.agent_info_expires_after_ms as u64;
        let dynamic_arcs = self.config.tuning_params.gossip_dynamic_arcs;
        let internal_sender = self.i_s.clone();
        Ok(async move {
            let urls = vec![TxUrl::try_from(local_url)?];
            let mut peer_data = Vec::with_capacity(agent_list.len());
            for (agent, arq) in agent_list {
                let input = UpdateAgentInfoInput {
                    expires_after,
                    space: space.clone(),
                    agent,
                    bootstrap_net,
                    arq,
                    urls: &urls,
                    evt_sender: &evt_sender,
                    internal_sender: &internal_sender,
                    network_type: network_type.clone(),
                    mdns_handles: &mut mdns_handles,
                    bootstrap_service: &bootstrap_service,
                    dynamic_arcs,
                };
                peer_data.push(update_single_agent_info(input).await?);
            }
            internal_sender
                .publish_agent_info_signed(PutAgentInfoSignedEvt { peer_data })
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
        let local_url = match self.ro_inner.get_local_url() {
            None => return Ok(async move { Ok(()) }.boxed().into()),
            Some(local_url) => local_url,
        };
        let space = self.space.clone();
        let bootstrap_net = self.ro_inner.bootstrap_net;
        let mut mdns_handles = self.mdns_handles.clone();
        let network_type = self.config.network_type.clone();
        let evt_sender = self.host_api.legacy.clone();
        let internal_sender = self.i_s.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        let expires_after = self.config.tuning_params.agent_info_expires_after_ms as u64;
        let dynamic_arcs = self.config.tuning_params.gossip_dynamic_arcs;
        let arc = self.get_agent_arq(&agent);

        Ok(async move {
            let urls = vec![TxUrl::try_from(local_url)?];
            let input = UpdateAgentInfoInput {
                expires_after,
                space: space.clone(),
                agent,
                bootstrap_net,
                arq: arc,
                urls: &urls,
                evt_sender: &evt_sender,
                internal_sender: &internal_sender,
                network_type: network_type.clone(),
                mdns_handles: &mut mdns_handles,
                bootstrap_service: &bootstrap_service,
                dynamic_arcs,
            };
            let peer_data = vec![update_single_agent_info(input).await?];
            internal_sender
                .publish_agent_info_signed(PutAgentInfoSignedEvt { peer_data })
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_publish_agent_info_signed(
        &mut self,
        input: PutAgentInfoSignedEvt,
    ) -> SpaceInternalHandlerResult<()> {
        let timeout = self.config.tuning_params.implicit_timeout();
        let tasks: Result<Vec<_>, _> = input
            .peer_data
            .iter()
            .map(|agent_info| {
                self.handle_broadcast(
                    self.space.clone(),
                    Arc::new(KitsuneBasis::new(agent_info.agent.0.clone())),
                    timeout,
                    BroadcastData::AgentInfo(agent_info.clone()),
                )
            })
            .collect();
        let ep_hnd = self.ro_inner.ep_hnd.clone();
        Ok(async move {
            futures::future::join(
                ep_hnd.broadcast(
                    &wire::Wire::PeerUnsolicited(crate::wire::PeerUnsolicited {
                        peer_list: input.peer_data,
                    }),
                    timeout,
                ),
                futures::future::join_all(tasks?),
            )
            .await
            .0?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_get_all_local_joined_agent_infos(
        &mut self,
    ) -> SpaceInternalHandlerResult<Vec<AgentInfoSigned>> {
        let agent_infos: Vec<AgentInfoSigned> = self
            .local_joined_agents
            .values()
            .filter_map(|maybe_agent_info| maybe_agent_info.as_ref())
            .cloned()
            .collect();

        Ok(async move { Ok(agent_infos) }.boxed().into())
    }

    fn handle_is_agent_local(
        &mut self,
        agent: Arc<KitsuneAgent>,
    ) -> SpaceInternalHandlerResult<bool> {
        let res = self.local_joined_agents.contains_key(&agent);
        Ok(async move { Ok(res) }.boxed().into())
    }

    fn handle_update_agent_arc(
        &mut self,
        agent: Arc<KitsuneAgent>,
        arq: Arq,
    ) -> SpaceInternalHandlerResult<()> {
        self.agent_arqs.insert(agent, arq);
        self.update_metric_exchange_arcset();
        Ok(async move { Ok(()) }.boxed().into())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    fn handle_incoming_delegate_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        basis: Arc<KitsuneBasis>,
        _to_agent: Arc<KitsuneAgent>,
        mod_idx: u32,
        mod_cnt: u32,
        data: BroadcastData,
    ) -> InternalHandlerResult<()> {
        // first, forward this incoming broadcast to all connected
        // local agents.
        let mut local_notify_events = Vec::new();
        let mut local_agent_info_events = Vec::new();
        let broadcast_source = match &data {
            BroadcastData::User(data) => {
                for agent in self.local_joined_agents.keys() {
                    if let Some(arc) = self.agent_arqs.get(agent) {
                        if arc.to_dht_arc_std().contains(basis.get_loc()) {
                            let fut = self.host_api.legacy.notify(
                                space.clone(),
                                agent.clone(),
                                data.clone(),
                            );
                            local_notify_events.push(async move {
                                if let Err(err) = fut.await {
                                    tracing::warn!(?err, "failed local broadcast");
                                }
                            });
                        }
                    }
                }

                None
            }
            BroadcastData::AgentInfo(agent_info) => {
                if self
                    .agent_arqs
                    .values()
                    .any(|arc| arc.to_dht_arc_std().contains(basis.get_loc()))
                {
                    let fut = self
                        .host_api
                        .legacy
                        .put_agent_info_signed(PutAgentInfoSignedEvt {
                            peer_data: vec![agent_info.clone()],
                        });
                    local_agent_info_events.push(async move {
                        if let Err(err) = fut.await {
                            tracing::warn!(?err, "failed local broadcast");
                        }
                    });
                }

                None
            }
            BroadcastData::Publish { source, .. } => {
                // Don't do anything here. This case is handled by the actor
                // invoking incoming_publish instead of
                // incoming_delegate_broadcast.
                Some(source.clone())
            }
        };

        // next, gather a list of agents covering this data to be
        // published to.
        let ro_inner = self.ro_inner.clone();
        let timeout = ro_inner.config.tuning_params.implicit_timeout();
        let fut =
            discover::search_remotes_covering_basis(ro_inner.clone(), basis.get_loc(), timeout);

        Ok(async move {
            futures::future::join_all(local_notify_events).await;
            futures::future::join_all(local_agent_info_events).await;

            let info_list = fut.await?;

            // for all agents in the gathered list, check the modulo params
            // i.e. if `agent.get_loc() % mod_cnt == mod_idx` we know we are
            // responsible for delegating the broadcast to that agent.
            let mut all = Vec::new();
            for info in info_list.into_iter().filter(|info| {
                info.agent.get_loc().as_u32() % mod_cnt == mod_idx
                    && Some(info.agent()) != broadcast_source
            }) {
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

    fn handle_incoming_publish(
        &mut self,
        space: KSpace,
        to_agent: KAgent,
        source: KAgent,
        op_hash_list: OpHashList,
        context: kitsune_p2p_fetch::FetchContext,
        maybe_delegate: MaybeDelegate,
    ) -> InternalHandlerResult<()> {
        let ro_inner = self.ro_inner.clone();

        let just_hashes = op_hash_list.iter().map(|s| s.data()).collect();

        Ok(async move {
            let have_data_list = match ro_inner
                .host_api
                .check_op_data(space.clone(), just_hashes, Some(context))
                .await
                .map_err(KitsuneP2pError::other)
            {
                Err(err) => {
                    tracing::warn!(?err);
                    return Err(err);
                }
                Ok(res) => res,
            };

            for (op_hash, have_data) in op_hash_list.into_iter().zip(have_data_list) {
                if have_data {
                    if let Some((basis, mod_idx, mod_cnt)) = &maybe_delegate {
                        ro_inner
                            .i_s
                            .incoming_delegate_broadcast(
                                space.clone(),
                                basis.clone(),
                                to_agent.clone(),
                                *mod_idx,
                                *mod_cnt,
                                BroadcastData::Publish {
                                    source: source.clone(),
                                    op_hash_list: vec![op_hash],
                                    context,
                                },
                            )
                            .await?;
                    }
                    continue;
                } else {
                    // Add this hash to our fetch queue.
                    ro_inner.fetch_pool.push(FetchPoolPush {
                        key: FetchKey::Op(op_hash.data()),
                        space: space.clone(),
                        source: FetchSource::Agent(source.clone()),
                        size: op_hash.maybe_size(),
                        context: Some(context),
                        transfer_method: TransferMethod::Publish,
                    });

                    ro_inner.host_api.handle_op_hash_received(
                        &space,
                        &op_hash,
                        TransferMethod::Publish,
                    );

                    // Register a callback if maybe_delegate.is_some()
                    // to invoke the delegation on receipt of data.
                    if let Some((basis, mod_idx, mod_cnt)) = &maybe_delegate {
                        ro_inner.clone().publish_pending_delegate(
                            op_hash.data(),
                            PendingDelegate {
                                space: space.clone(),
                                basis: basis.clone(),
                                to_agent: to_agent.clone(),
                                mod_idx: *mod_idx,
                                mod_cnt: *mod_cnt,
                                data: BroadcastData::Publish {
                                    source: source.clone(),
                                    op_hash_list: vec![op_hash],
                                    context,
                                },
                            },
                        );
                    }
                }
            }

            Ok(())
        }
        .boxed()
        .into())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    fn handle_notify(&mut self, to_agent: KAgent, data: wire::Wire) -> InternalHandlerResult<()> {
        let ro_inner = self.ro_inner.clone();
        let timeout = ro_inner.config.tuning_params.implicit_timeout();

        Ok(async move {
            match discover::search_and_discover_peer_connect(
                ro_inner.clone(),
                to_agent.clone(),
                timeout,
            )
            .await
            {
                discover::PeerDiscoverResult::OkShortcut => {
                    tracing::warn!("no reason to notify ourselves");
                }
                discover::PeerDiscoverResult::OkRemote { url: _, con_hnd } => {
                    if let Err(err) = con_hnd.notify(&data, timeout).await {
                        tracing::debug!(?err);
                    }
                }
                discover::PeerDiscoverResult::Err(err) => {
                    tracing::debug!(?err);
                }
            }

            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_resolve_publish_pending_delegates(
        &mut self,
        _space: KSpace,
        op_hash: KOpHash,
    ) -> InternalHandlerResult<()> {
        self.ro_inner.resolve_publish_pending_delegate(op_hash);

        unit_ok_fut()
    }

    fn handle_incoming_gossip(
        &mut self,
        _space: Arc<KitsuneSpace>,
        con: MetaNetCon,
        remote_url: String,
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

    fn handle_incoming_metric_exchange(
        &mut self,
        _space: Arc<KitsuneSpace>,
        msgs: Vec<MetricExchangeMsg>,
    ) -> InternalHandlerResult<()> {
        self.ro_inner.metric_exchange.write().ingest_msgs(msgs);
        unit_ok_fut()
    }

    fn handle_new_con(&mut self, url: String, con: MetaNetCon) -> InternalHandlerResult<()> {
        self.ro_inner.metric_exchange.write().new_con(url, con);
        unit_ok_fut()
    }

    fn handle_del_con(&mut self, url: String) -> InternalHandlerResult<()> {
        self.ro_inner.metric_exchange.write().del_con(url);
        unit_ok_fut()
    }
}

struct UpdateAgentInfoInput<'borrow> {
    expires_after: u64,
    space: Arc<KitsuneSpace>,
    agent: Arc<KitsuneAgent>,
    bootstrap_net: BootstrapNet,
    arq: Arq,
    urls: &'borrow Vec<TxUrl>,
    evt_sender: &'borrow futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    internal_sender: &'borrow ghost_actor::GhostSender<SpaceInternal>,
    network_type: NetworkType,
    mdns_handles: &'borrow mut HashMap<Vec<u8>, Arc<AtomicBool>>,
    bootstrap_service: &'borrow Option<Url2>,
    dynamic_arcs: bool,
}

async fn update_arc_length(
    evt_sender: &futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    space: Arc<KitsuneSpace>,
    arq: &mut Arq,
) -> KitsuneP2pResult<()> {
    let dim = SpaceDimension::standard();
    let arc = arq.to_dht_arc(dim);
    let view = evt_sender.query_peer_density(space.clone(), arc).await?;

    let cov_before = arq.coverage(dim) * 100.0;
    tracing::trace!("Updating arc for space {:?}:", space);

    #[cfg(feature = "test_utils")]
    tracing::trace!("Before: {:2.1}% |{}|", cov_before, arq.to_ascii(dim, 64));

    view.update_arq(arq);

    let cov_after = arq.coverage(dim) * 100.0;

    #[cfg(feature = "test_utils")]
    tracing::trace!("After:  {:2.1}% |{}|", cov_after, arq.to_ascii(dim, 64));

    tracing::trace!("Diff: {:-2.2}%", cov_after - cov_before);

    Ok(())
}

// TODO document me
async fn update_single_agent_info(
    input: UpdateAgentInfoInput<'_>,
) -> KitsuneP2pResult<AgentInfoSigned> {
    let UpdateAgentInfoInput {
        expires_after,
        space,
        agent,
        bootstrap_net,
        mut arq,
        urls,
        evt_sender,
        internal_sender,
        network_type,
        mdns_handles,
        bootstrap_service,
        dynamic_arcs,
    } = input;

    if dynamic_arcs {
        update_arc_length(evt_sender, space.clone(), &mut arq).await?;
    }

    // Update the agents arc through the internal sender.
    internal_sender.update_agent_arc(agent.clone(), arq).await?;

    let signed_at_ms = kitsune_p2p_bootstrap_client::now_once(None, bootstrap_net).await?;
    let expires_at_ms = signed_at_ms + expires_after;

    let agent_info_signed = AgentInfoSigned::sign(
        space.clone(),
        agent.clone(),
        arq,
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

    // Write to the local peer store. The agent info will also be stored via a local broadcast,
    // except in the case of a zero arc where it gets filtered out, so we store it here too
    // to be sure it makes it into the local store.
    put_local_agent_info(evt_sender.clone(), agent_info_signed.clone()).await?;

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
                let space_b64 = base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(&space[..]);
                let agent_b64 = base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(&agent[..]);
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
            kitsune_p2p_bootstrap_client::put(
                bootstrap_service.clone(),
                agent_info_signed.clone(),
                bootstrap_net,
            )
            .await?;
        }
    }
    Ok(agent_info_signed)
}

use crate::spawn::actor::space::agent_info_update::AgentInfoUpdateTask;
use crate::spawn::actor::space::bootstrap_task::BootstrapTask;
use ghost_actor::dependencies::must_future::MustBoxFuture;

impl ghost_actor::GhostControlHandler for Space {
    fn handle_ghost_actor_shutdown(mut self) -> MustBoxFuture<'static, ()> {
        async move {
            // The line below was added when migrating to rust edition 2021, per
            // https://doc.rust-lang.org/edition-guide/rust-2021/disjoint-capture-in-closures.html#migration
            let _ = &self;
            self.ro_inner.metric_exchange.write().shutdown();

            use futures::sink::SinkExt;
            // this is a curtesy, ok if fails
            let _ = self.host_api.legacy.close().await;
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
    fn handle_join(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        maybe_agent_info: Option<AgentInfoSigned>,
        initial_arq: Option<Arq>,
    ) -> KitsuneP2pHandlerResult<()> {
        tracing::debug!(?space, ?agent, ?initial_arq, "handle_join");

        match initial_arq {
            // This agent has been on the network before and has an arc stored on the Kitsune host.
            // Respect the current storage settings and apply this arc.
            Some(initial_arq) => {
                self.agent_arqs.insert(agent.clone(), initial_arq);
            }
            // This agent is new to the network and has no arc stored on the Kitsune host.
            // Get a default arc for the agent
            None => {
                self.agent_arqs
                    .insert(agent.clone(), self.default_arc_for_agent(agent.clone()));
            }
        }

        self.local_joined_agents
            .insert(agent.clone(), maybe_agent_info);
        for module in self.gossip_mod.values() {
            module.local_agent_join(agent.clone());
        }
        let fut = self.i_s.update_single_agent_info(agent);
        let evt_sender = self.host_api.legacy.clone();
        match self.config.network_type {
            NetworkType::QuicMdns => {
                // Listen to MDNS service that has that space as service type
                let space_b64 = base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(&space[..]);
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

        Ok(fut.boxed().into())
    }

    fn handle_leave(
        &mut self,
        _space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        self.local_joined_agents.remove(&agent);
        self.agent_arqs.remove(&agent);
        self.update_metric_exchange_arcset();
        for module in self.gossip_mod.values() {
            module.local_agent_leave(agent.clone());
        }
        self.publish_leave_agent_info(agent)
    }

    fn handle_rpc_single(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
        timeout_ms: Option<u64>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.host_api.legacy.clone();

        let timeout_ms = match timeout_ms {
            None | Some(0) => self.config.tuning_params.default_rpc_single_timeout_ms as u64,
            _ => timeout_ms.unwrap(),
        };
        let timeout = KitsuneTimeout::from_millis(timeout_ms);

        let start = tokio::time::Instant::now();

        let discover_fut = discover::search_and_discover_peer_connect(
            self.ro_inner.clone(),
            to_agent.clone(),
            timeout,
        );

        let metrics = self.ro_inner.metrics.clone();

        Ok(async move {
            match discover_fut.await {
                discover::PeerDiscoverResult::OkShortcut => {
                    // reflect this request locally
                    evt_sender.call(space, to_agent, payload).await
                }
                discover::PeerDiscoverResult::OkRemote { con_hnd, .. } => {
                    let payload = wire::Wire::call(space.clone(), to_agent.clone(), payload.into());
                    let res = con_hnd.request(&payload, timeout).await?;
                    match res {
                        wire::Wire::Failure(wire::Failure { reason }) => {
                            metrics
                                .write()
                                .record_reachability_event(false, [&to_agent]);
                            metrics
                                .write()
                                .record_latency_micros(start.elapsed().as_micros(), [&to_agent]);
                            Err(reason.into())
                        }
                        wire::Wire::CallResp(wire::CallResp { data }) => {
                            metrics.write().record_reachability_event(true, [&to_agent]);
                            metrics
                                .write()
                                .record_latency_micros(start.elapsed().as_micros(), [&to_agent]);
                            Ok(data.into())
                        }
                        r => {
                            metrics
                                .write()
                                .record_reachability_event(false, [&to_agent]);
                            metrics
                                .write()
                                .record_latency_micros(start.elapsed().as_micros(), [&to_agent]);
                            Err(format!("invalid response: {:?}", r).into())
                        }
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
        let local_joined_agents = self.local_joined_agents.keys().cloned().collect();
        let fut =
            rpc_multi_logic::handle_rpc_multi(input, self.ro_inner.clone(), local_joined_agents);
        Ok(fut.boxed().into())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    fn handle_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        basis: Arc<KitsuneBasis>,
        timeout: KitsuneTimeout,
        data: BroadcastData,
    ) -> KitsuneP2pHandlerResult<()> {
        // first, forward this data to all connected local agents.
        let mut local_notify_events = Vec::new();
        let mut local_agent_info_events = Vec::new();
        match &data {
            BroadcastData::User(data) => {
                for (agent_key, _agent_info) in self.local_joined_agents.iter() {
                    if let Some(arq) = self.agent_arqs.get(agent_key) {
                        if arq.to_dht_arc_std().contains(basis.get_loc()) {
                            let fut = self.host_api.legacy.notify(
                                space.clone(),
                                agent_key.clone(),
                                data.clone(),
                            );
                            local_notify_events.push(async move {
                                if let Err(err) = fut.await {
                                    tracing::warn!(?err, "failed local broadcast");
                                }
                            });
                        }
                    }
                }
            }
            BroadcastData::AgentInfo(agent_info) => {
                if self
                    .agent_arqs
                    .values()
                    .any(|arc| arc.to_dht_arc_std().contains(basis.get_loc()))
                {
                    let fut =
                        put_local_agent_info(self.host_api.legacy.clone(), agent_info.clone());
                    local_agent_info_events.push(async move {
                        if let Err(err) = fut.await {
                            tracing::warn!(?err, "failed local broadcast");
                        }
                    });
                }
            }
            BroadcastData::Publish { .. } => {
                // There is nothing to do here!
                // *We* are the node publishing
                // so we already have these hashes : )
            }
        }

        // then, find a list of agents in a potentially remote neighborhood
        // that should be responsible for holding the data.
        let ro_inner = self.ro_inner.clone();
        let discover_fut =
            discover::search_remotes_covering_basis(ro_inner.clone(), basis.get_loc(), timeout);
        Ok(async move {
            futures::future::join_all(local_notify_events).await;
            futures::future::join_all(local_agent_info_events).await;

            // NOTE
            // Holochain currently does most of its testing without any remote
            // nodes if we do this inline, it takes us to the 30 second timeout
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

                // Determine the total number of nodes we'll be publishing to.
                // We'll make each remote responsible for a subset of delegate
                // broadcasting by having them apply the formula:
                // `agent.get_loc() % mod_cnt == mod_idx` -- if true,
                // they'll be responsible for forwarding the data to that node.
                let mod_cnt = con_list.len();
                for (mod_idx, (agent, con_hnd)) in con_list.into_iter().enumerate() {
                    // build our delegate message
                    let data = wire::Wire::delegate_broadcast(
                        space.clone(),
                        basis.clone(),
                        agent,
                        mod_idx as u32,
                        mod_cnt as u32,
                        data.clone(),
                    );

                    // notify the remote node
                    all.push(async move {
                        if let Err(err) = con_hnd.notify(&data, timeout).await {
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

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    fn handle_targeted_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        agents: Vec<Arc<KitsuneAgent>>,
        timeout: KitsuneTimeout,
        payload: Vec<u8>,
        drop_at_limit: bool,
    ) -> KitsuneP2pHandlerResult<()> {
        let evt_sender = self.host_api.legacy.clone();
        let ro_inner = self.ro_inner.clone();
        Ok(async move {
            for agent in agents {
                let task_permit = if drop_at_limit {
                    match ro_inner.parallel_notify_permit.clone().try_acquire_owned() {
                        Ok(p) => Some(p),
                        Err(_) => {
                            tracing::debug!(
                                "Too many outstanding notifies, dropping notify to {:?}",
                                agent
                            );
                            continue;
                        }
                    }
                } else {
                    // limit spawns with semaphore.
                    ro_inner
                        .parallel_notify_permit
                        .clone()
                        .acquire_owned()
                        .await
                        .ok()
                };
                let space = space.clone();
                let payload = payload.clone();
                let evt_sender = evt_sender.clone();
                let ro_inner = ro_inner.clone();
                tokio::task::spawn(async move {
                    let discover_result = discover::search_and_discover_peer_connect(
                        ro_inner.clone(),
                        agent.clone(),
                        timeout,
                    )
                    .await;
                    match discover_result {
                        discover::PeerDiscoverResult::OkShortcut => {
                            // reflect this request locally
                            evt_sender
                                .notify(space, agent, payload)
                                .map(|r| {
                                    if let Err(e) = r {
                                        tracing::error!(
                                            "Failed to broadcast to local agent because: {:?}",
                                            e
                                        )
                                    }
                                })
                                .await;
                        }
                        discover::PeerDiscoverResult::OkRemote { con_hnd, .. } => {
                            let payload =
                                wire::Wire::broadcast(space, agent, BroadcastData::User(payload));
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
                                .await;
                        }
                        discover::PeerDiscoverResult::Err(e) => {
                            async move {
                                tracing::info!(
                                    "Failed to discover connection for {:?} because: {:?}",
                                    agent,
                                    e
                                );
                            }
                            .await
                        }
                    }

                    drop(task_permit);
                });
            }
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
        basis: Arc<KitsuneBasis>,
    ) -> KitsuneP2pHandlerResult<bool> {
        tracing::info!("Have local joined agents {:?}", self.local_joined_agents);
        let loc = basis.get_loc();
        tracing::info!(
            "Doing basis check against num arcs {}",
            self.agent_arqs.len()
        );
        let r = self
            .agent_arqs
            .values()
            .any(|arq| arq.to_dht_arc_std().contains(loc));
        Ok(async move { Ok(r) }.boxed().into())
    }

    fn handle_dump_network_metrics(
        &mut self,
        _space: Option<Arc<KitsuneSpace>>,
    ) -> KitsuneP2pHandlerResult<serde_json::Value> {
        let space = self.ro_inner.space.clone();
        let metrics = self.ro_inner.metrics.read().dump();
        Ok(async move {
            Ok(serde_json::json!({
                "space": space.to_string(),
                "metrics": metrics,
            }))
        }
        .boxed()
        .into())
    }

    fn handle_dump_network_stats(&mut self) -> KitsuneP2pHandlerResult<serde_json::Value> {
        // call handled by parent actor and never delegated to spaces
        unreachable!()
    }

    fn handle_get_diagnostics(
        &mut self,
        _space: KSpace,
    ) -> KitsuneP2pHandlerResult<KitsuneDiagnostics> {
        let diagnostics = KitsuneDiagnostics {
            metrics: self.ro_inner.metrics.clone(),
            fetch_pool: self.ro_inner.fetch_pool.clone().into(),
        };
        Ok(async move { Ok(diagnostics) }.boxed().into())
    }
}

pub(crate) struct PendingDelegate {
    pub(crate) space: KSpace,
    pub(crate) basis: KBasis,
    pub(crate) to_agent: KAgent,
    pub(crate) mod_idx: u32,
    pub(crate) mod_cnt: u32,
    pub(crate) data: BroadcastData,
}

pub(crate) struct SpaceReadOnlyInner {
    pub(crate) local_url: Arc<std::sync::Mutex<Option<String>>>,
    pub(crate) space: Arc<KitsuneSpace>,
    #[allow(dead_code)]
    pub(crate) i_s: ghost_actor::GhostSender<SpaceInternal>,
    pub(crate) host_api: HostApiLegacy,
    pub(crate) ep_hnd: MetaNet,
    #[allow(dead_code)]
    pub(crate) config: Arc<KitsuneP2pConfig>,
    pub(crate) bootstrap_net: BootstrapNet,
    pub(crate) parallel_notify_permit: Arc<tokio::sync::Semaphore>,
    pub(crate) metrics: MetricsSync,
    pub(crate) metric_exchange: MetricExchangeSync,
    pub(crate) publish_pending_delegates: parking_lot::Mutex<HashMap<KOpHash, PendingDelegate>>,
    #[allow(dead_code)]
    pub(crate) fetch_pool: FetchPool,
}

impl SpaceReadOnlyInner {
    pub(crate) fn get_local_url(&self) -> Option<String> {
        self.local_url.lock().unwrap().clone()
    }

    pub(crate) fn publish_pending_delegate(
        self: Arc<Self>,
        op_hash: KOpHash,
        pending_delegate: PendingDelegate,
    ) {
        {
            let this = self.clone();
            let op_hash = op_hash.clone();
            tokio::task::spawn(async move {
                tokio::time::sleep(
                    this.config
                        .tuning_params
                        .implicit_timeout()
                        .time_remaining(),
                )
                .await;

                this.publish_pending_delegates.lock().remove(&op_hash);
            });
        }

        self.publish_pending_delegates
            .lock()
            .insert(op_hash, pending_delegate);
    }

    pub(crate) fn resolve_publish_pending_delegate(&self, op_hash: KOpHash) {
        if let Some(PendingDelegate {
            space,
            basis,
            to_agent,
            mod_idx,
            mod_cnt,
            data,
        }) = self.publish_pending_delegates.lock().remove(&op_hash)
        {
            let i_s = self.i_s.clone();
            tokio::task::spawn(async move {
                let _ = i_s
                    .incoming_delegate_broadcast(space, basis, to_agent, mod_idx, mod_cnt, data)
                    .await;
            });
        }
    }
}

/// A Kitsune P2p Node can track multiple "spaces" -- Non-interacting namespaced
/// areas that share common transport infrastructure for communication.
pub(crate) struct Space {
    pub(crate) ro_inner: Arc<SpaceReadOnlyInner>,
    pub(crate) space: Arc<KitsuneSpace>,
    pub(crate) i_s: ghost_actor::GhostSender<SpaceInternal>,
    pub(crate) host_api: HostApiLegacy,
    pub(crate) local_joined_agents: HashMap<Arc<KitsuneAgent>, Option<AgentInfoSigned>>,
    pub(crate) agent_arqs: HashMap<Arc<KitsuneAgent>, Arq>,
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
        i_s: ghost_actor::GhostSender<SpaceInternal>,
        host_api: HostApiLegacy,
        ep_hnd: MetaNet,
        config: Arc<KitsuneP2pConfig>,
        bootstrap_net: BootstrapNet,
        bandwidth_throttles: BandwidthThrottles,
        parallel_notify_permit: Arc<tokio::sync::Semaphore>,
        fetch_pool: FetchPool,
        local_url: Arc<std::sync::Mutex<Option<String>>>,
    ) -> Self {
        let metrics = MetricsSync::default();

        {
            let space = space.clone();
            let metrics = metrics.clone();
            let host = host_api.clone();
            tokio::task::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(
                        HISTORICAL_METRIC_RECORD_FREQ_MS,
                    ))
                    .await;

                    let records = metrics.read().dump_historical();

                    let _ = host.record_metrics(space.clone(), records).await;
                }
            });
        }

        let metric_exchange = MetricExchangeSync::spawn(
            space.clone(),
            config.tuning_params.clone(),
            host_api.clone(),
            metrics.clone(),
        );

        let gossip_mod = if config.tuning_params.arc_clamping() == Some(ArqClamping::Empty) {
            // If arcs are clamped to zero, then completely disable gossip for this node.
            // Gossip will never resume
            tracing::info!("Gossip is disabled due to arc_clamping setting");
            HashMap::new()
        } else {
            config
                .tuning_params
                .gossip_strategy
                .split(',')
                .flat_map(|module| match module {
                    "sharded-gossip" => {
                        let mut gossips = vec![];
                        if !config.tuning_params.disable_recent_gossip {
                            gossips.push((
                                GossipModuleType::ShardedRecent,
                                crate::gossip::sharded_gossip::recent_factory(
                                    bandwidth_throttles.recent(),
                                ),
                            ));
                        }
                        if !config.tuning_params.disable_historical_gossip {
                            gossips.push((
                                GossipModuleType::ShardedHistorical,
                                crate::gossip::sharded_gossip::historical_factory(
                                    bandwidth_throttles.historical(),
                                ),
                            ));
                        }
                        gossips
                    }
                    "none" => vec![],
                    _ => {
                        panic!("unknown gossip strategy: {}", module);
                    }
                })
                .map(|(module, factory)| {
                    (
                        module,
                        factory.spawn_gossip_task(
                            config.clone(),
                            space.clone(),
                            ep_hnd.clone(),
                            host_api.clone(),
                            metrics.clone(),
                            fetch_pool.clone(),
                        ),
                    )
                })
                .collect()
        };

        AgentInfoUpdateTask::spawn(
            i_s.clone(),
            std::time::Duration::from_millis(
                config.tuning_params.gossip_agent_info_update_interval_ms as u64,
            ),
        );

        if let NetworkType::QuicBootstrap = &config.network_type {
            // spawn the periodic bootstrap pull
            BootstrapTask::spawn(
                i_s.clone(),
                host_api.legacy.clone(),
                space.clone(),
                config.bootstrap_service.clone(),
                bootstrap_net,
                config
                    .tuning_params
                    .bootstrap_check_delay_backoff_multiplier,
                config.tuning_params.bootstrap_max_delay_s,
            );
        }

        let ro_inner = Arc::new(SpaceReadOnlyInner {
            local_url,
            space: space.clone(),
            i_s: i_s.clone(),
            host_api: host_api.clone(),
            ep_hnd,
            config: config.clone(),
            bootstrap_net,
            parallel_notify_permit,
            metrics,
            metric_exchange,
            publish_pending_delegates: parking_lot::Mutex::new(HashMap::new()),
            fetch_pool,
        });

        Self {
            ro_inner,
            space,
            i_s,
            host_api,
            local_joined_agents: HashMap::new(),
            agent_arqs: HashMap::new(),
            config,
            mdns_handles: HashMap::new(),
            mdns_listened_spaces: HashSet::new(),
            gossip_mod,
        }
    }

    fn update_metric_exchange_arcset(&mut self) {
        let arc_set = self
            .agent_arqs
            .values()
            .map(|a| DhtArcSet::from_interval(DhtArcRange::from(a.to_dht_arc_std())))
            .fold(DhtArcSet::new_empty(), |a, i| a.union(&i));
        self.ro_inner.metric_exchange.write().update_arcset(arc_set);
    }

    fn publish_leave_agent_info(
        &mut self,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space = self.space.clone();
        let network_type = self.config.network_type.clone();
        let bootstrap_net = self.ro_inner.bootstrap_net;
        let evt_sender = self.host_api.legacy.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        let expires_after = self.config.tuning_params.agent_info_expires_after_ms as u64;
        let host = self.host_api.clone();

        Ok(async move {
            let signed_at_ms = kitsune_p2p_bootstrap_client::now_once(None, bootstrap_net).await?;
            let expires_at_ms = signed_at_ms + expires_after;
            let agent_info_signed = AgentInfoSigned::sign(
                space.clone(),
                agent.clone(),
                ArqSize::empty(), // no storage arc
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

            // TODO: at some point, we should not remove agents who have left, but rather
            // there should be a flag indicating they have left. The removed agent may just
            // get re-gossiped to another local agent in the same space, defeating the purpose.
            host.remove_agent_info_signed(GetAgentInfoSignedEvt { space, agent })
                .await
                .map_err(KitsuneP2pError::other)?;

            // Push to the network as well
            match network_type {
                NetworkType::QuicMdns => tracing::warn!("NOT publishing leaves to mdns"),
                NetworkType::QuicBootstrap => {
                    match kitsune_p2p_bootstrap_client::put(
                        bootstrap_service.clone(),
                        agent_info_signed,
                        bootstrap_net,
                    )
                    .await {
                        Ok(_) => {
                            tracing::debug!("Successfully publish agent info to the bootstrap service");
                        }
                        Err(err) => {
                            tracing::info!(?err, "Failed to publish agent info to the bootstrap service while leaving");
                        }
                    }
                }
            }

            Ok(())
        }
        .boxed()
        .into())
    }

    /// Get the existing agent storage arc or create a new one.
    fn get_agent_arq(&self, agent: &Arc<KitsuneAgent>) -> Arq {
        // TODO: We are simply setting the initial arc to full.
        // In the future we may want to do something more intelligent.
        //
        // In the case an initial_arc is passed into the join request,
        // handle_join will initialize this agent_arcs map to that value.
        self.agent_arqs.get(agent).cloned().unwrap_or_else(|| {
            tracing::warn!("Using default arc for agent, this should be configured by the host or initialised on network join");
            self.default_arc_for_agent((*agent).clone())
        })
    }

    /// Get the default arc for an agent.
    ///
    /// The default arc depends on the Kitsune tuning parameters.
    /// - Either arc clamping is set to empty and the arc is empty, or
    /// - The arc clamping is set to full or not set, in which cases the arc is full.
    fn default_arc_for_agent(&self, agent: Arc<KitsuneAgent>) -> Arq {
        let dim = SpaceDimension::standard();
        match self.config.tuning_params.arc_clamping() {
            Some(ArqClamping::Empty) => Arq::new_empty(dim, agent.get_loc()),
            Some(ArqClamping::Full) | None => {
                let strat = self.config.tuning_params.to_arq_strat();
                Arq::new_full_max(dim, &strat, agent.get_loc())
            }
        }
    }
}

/// Function to add local agent info to the store.
/// There's nothing special about this method, it's just helpful to
/// semantically see the situations where local info is being added.
async fn put_local_agent_info(
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    agent_info: AgentInfoSigned,
) -> KitsuneP2pResult<()> {
    evt_sender
        .put_agent_info_signed(PutAgentInfoSignedEvt {
            peer_data: vec![agent_info],
        })
        .await?;
    Ok(())
}
