// this is largely a passthrough that routes to a specific space handler

use crate::actor;
use crate::actor::*;
use crate::event::*;
use crate::gossip::sharded_gossip::BandwidthThrottles;
use crate::metrics::KitsuneMetrics;
use crate::types::gossip::GossipModuleType;
use crate::*;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_proxy::ProxyUrl;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::async_lazy::AsyncLazy;
use kitsune_p2p_types::tx2::tx2_adapter::AdapterFactory;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

/// The bootstrap service is much more thoroughly documented in the default service implementation.
/// See https://github.com/holochain/bootstrap
mod bootstrap;
mod discover;
mod space;
use ghost_actor::dependencies::tracing;
use space::*;

type EvtRcv = futures::channel::mpsc::Receiver<KitsuneP2pEvent>;
type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KBasis = Arc<KitsuneBasis>;
type WireConHnd = Tx2ConHnd<wire::Wire>;
type Payload = Box<[u8]>;

ghost_actor::ghost_chan! {
    pub(crate) chan Internal<crate::KitsuneP2pError> {
        /// Register space event handler
        fn register_space_event_handler(recv: EvtRcv) -> ();

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
        fn incoming_gossip(space: KSpace, con: WireConHnd, remote_url: kitsune_p2p_types::tx2::tx2_utils::TxUrl, data: Payload, module_type: crate::types::gossip::GossipModuleType) -> ();
    }
}

pub(crate) struct KitsuneP2pActor {
    this_addr: url2::Url2,
    channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
    internal_sender: ghost_actor::GhostSender<Internal>,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    #[allow(clippy::type_complexity)]
    spaces: HashMap<
        Arc<KitsuneSpace>,
        AsyncLazy<(
            ghost_actor::GhostSender<KitsuneP2p>,
            ghost_actor::GhostSender<space::SpaceInternal>,
        )>,
    >,
    config: Arc<KitsuneP2pConfig>,
    bandwidth_throttles: BandwidthThrottles,
    parallel_notify_permit: Arc<tokio::sync::Semaphore>,
}

impl KitsuneP2pActor {
    pub async fn new(
        config: KitsuneP2pConfig,
        tls_config: kitsune_p2p_proxy::TlsConfig,
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        internal_sender: ghost_actor::GhostSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
        mock_network: Option<AdapterFactory>,
    ) -> KitsuneP2pResult<Self> {
        crate::types::metrics::init();

        let mut tx2_conf = config.to_tx2().map_err(KitsuneP2pError::other)?;

        let is_mock = mock_network.is_some();
        if let Some(mock_network) = mock_network {
            tx2_conf.backend = KitsuneP2pTx2Backend::Mock { mock_network };
        }

        // set up our backend based on config
        let (f, bind_to) = match tx2_conf.backend {
            KitsuneP2pTx2Backend::Mem => {
                let mut conf = MemConfig::default();
                conf.tls = Some(tls_config.clone());
                conf.tuning_params = Some(config.tuning_params.clone());
                (
                    tx2_mem_adapter(conf)
                        .await
                        .map_err(KitsuneP2pError::other)?,
                    "none:".into(),
                )
            }
            KitsuneP2pTx2Backend::Quic { bind_to } => {
                let mut conf = QuicConfig::default();
                conf.tls = Some(tls_config.clone());
                conf.tuning_params = Some(config.tuning_params.clone());
                (
                    tx2_quic_adapter(conf)
                        .await
                        .map_err(KitsuneP2pError::other)?,
                    bind_to,
                )
            }
            KitsuneP2pTx2Backend::Mock { mock_network } => (mock_network, "none:".into()),
        };

        // convert to frontend
        let f = tx2_pool_promote(f, config.tuning_params.clone());

        // wrap in proxy
        let f = if !is_mock {
            let mut conf = kitsune_p2p_proxy::tx2::ProxyConfig::default();
            conf.tuning_params = Some(config.tuning_params.clone());
            let f = tx2_proxy(f, conf)?;
            f
        } else {
            f
        };

        let metrics = Tx2ApiMetrics::default().set_write_len(|d, l| {
            let t = match d {
                "Wire::Failure" => KitsuneMetrics::Failure,
                "Wire::Call" => KitsuneMetrics::Call,
                "Wire::CallResp" => KitsuneMetrics::CallResp,
                "Wire::Notify" => KitsuneMetrics::Notify,
                "Wire::NotifyResp" => KitsuneMetrics::NotifyResp,
                "Wire::Gossip" => KitsuneMetrics::Gossip,
                "Wire::PeerGet" => KitsuneMetrics::PeerGet,
                "Wire::PeerGetResp" => KitsuneMetrics::PeerGetResp,
                "Wire::PeerQuery" => KitsuneMetrics::PeerQuery,
                "Wire::PeerQueryResp" => KitsuneMetrics::PeerQueryResp,
                _ => return,
            };
            KitsuneMetrics::count(t, l);
        });

        // wrap in api
        let f = tx2_api(f, metrics);

        // bind local endpoint
        let ep = f
            .bind(bind_to, config.tuning_params.implicit_timeout())
            .await
            .map_err(KitsuneP2pError::other)?;

        // capture endpoint handle
        let ep_hnd = ep.handle().clone();

        // if we should be proxying - set up the proxy connect retry / proxy addr
        let this_addr = if let Some(use_proxy) = tx2_conf.use_proxy {
            let local = ep_hnd.local_addr().map_err(KitsuneP2pError::other)?;
            let this_digest = ProxyUrl::from(local.as_str()).digest();
            let proxy_url = ProxyUrl::from(use_proxy.as_str());

            // spawn logic that will attempt to keep us connected to the proxy
            let ep_hnd = ep_hnd.clone();
            let tuning_params = config.tuning_params.clone();
            tokio::task::spawn(async move {
                let mut con: Option<Tx2ConHnd<wire::Wire>> = None;
                loop {
                    // see if we need a new connection to the proxy
                    if con.is_none() || con.as_ref().unwrap().is_closed() {
                        if ep_hnd.is_closed() {
                            break;
                        }
                        match ep_hnd
                            .get_connection(use_proxy.clone(), tuning_params.implicit_timeout())
                            .await
                        {
                            Ok(c) => {
                                con = Some(c);
                            }
                            Err(e) => {
                                tracing::warn!("failure to establish proxy connection: {:?}", e);
                            }
                        }
                    }

                    // this is very naive... just running every 5 seconds
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                tracing::warn!("proxy con refresh loop shutdown");
            });

            ProxyUrl::new(proxy_url.as_base().as_str(), this_digest)
                .unwrap()
                .as_str()
                .into()
        } else {
            ep_hnd.local_addr().map_err(KitsuneP2pError::other)?
        };

        tracing::info!("this_addr: {}", this_addr);

        let i_s = internal_sender.clone();
        tokio::task::spawn({
            let evt_sender = evt_sender.clone();
            let tuning_params = config.tuning_params.clone();
            async move {
                ep.for_each_concurrent(tuning_params.concurrent_limit_per_thread, move |event| {
                    let evt_sender = evt_sender.clone();
                    let tuning_params = tuning_params.clone();
                    let i_s = i_s.clone();
                    async move {
                        macro_rules! resp {
                            ($r:expr, $e:expr) => {
                                // this can only error as channel closed
                                // it would be noise to output tracing errors
                                let _ = $r.respond($e, tuning_params.implicit_timeout()).await;
                            };
                        }

                        let evt_sender = &evt_sender;
                        use tx2_api::Tx2EpEvent::*;
                        #[allow(clippy::single_match)]
                        match event {
                            IncomingRequest(Tx2EpIncomingRequest { data, respond, .. }) => {
                                match data {
                                    wire::Wire::Call(wire::Call {
                                        space,
                                        from_agent,
                                        to_agent,
                                        data,
                                        ..
                                    }) => {
                                        let res = match evt_sender
                                            .call(space, to_agent, from_agent, data.into())
                                            .await
                                        {
                                            Err(err) => {
                                                let reason = format!("{:?}", err);
                                                let fail = wire::Wire::failure(reason);
                                                resp!(respond, fail);
                                                return;
                                            }
                                            Ok(r) => r,
                                        };
                                        let resp = wire::Wire::call_resp(res.into());
                                        resp!(respond, resp);
                                    }
                                    wire::Wire::PeerGet(wire::PeerGet { space, agent }) => {
                                        if let Ok(Some(agent_info_signed)) = evt_sender
                                            .get_agent_info_signed(GetAgentInfoSignedEvt {
                                                space,
                                                agent,
                                            })
                                            .await
                                        {
                                            let resp = wire::Wire::peer_get_resp(agent_info_signed);
                                            resp!(respond, resp);
                                        } else {
                                            let resp = wire::Wire::failure("no such agent".into());
                                            resp!(respond, resp);
                                        }
                                    }
                                    wire::Wire::PeerQuery(wire::PeerQuery { space, basis_loc }) => {
                                        // this *does* go over the network...
                                        // so we don't want it to be too many
                                        const LIMIT: u32 = 8;
                                        let query = QueryAgentsEvt::new(space)
                                            .near_basis(basis_loc)
                                            .limit(LIMIT);
                                        match evt_sender.query_agents(query).await {
                                            Ok(list) if !list.is_empty() => {
                                                let resp = wire::Wire::peer_query_resp(list);
                                                resp!(respond, resp);
                                            }
                                            res => {
                                                let resp = wire::Wire::failure(format!(
                                                    "error getting agents: {:?}",
                                                    res
                                                ));
                                                resp!(respond, resp);
                                            }
                                        }
                                    }
                                    data => unimplemented!("{:?}", data),
                                }
                            }
                            IncomingNotify(Tx2EpIncomingNotify { con, data, url, .. }) => {
                                match data {
                                    wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                                        space,
                                        basis,
                                        to_agent,
                                        mod_idx,
                                        mod_cnt,
                                        data,
                                    }) => {
                                        // one might be tempted to notify here
                                        // as in Broadcast below... but we
                                        // notify all relevent agents inside
                                        // the space incoming_delegate_broadcast
                                        // handler.
                                        if let Err(err) = i_s
                                            .incoming_delegate_broadcast(
                                                space, basis, to_agent, mod_idx, mod_cnt, data,
                                            )
                                            .await
                                        {
                                            tracing::warn!(
                                                ?err,
                                                "failed to handle incoming delegate broadcast"
                                            );
                                        }
                                    }
                                    wire::Wire::Broadcast(wire::Broadcast {
                                        space,
                                        to_agent,
                                        data,
                                        ..
                                    }) => {
                                        if let Err(err) = evt_sender
                                            .notify(space, to_agent.clone(), to_agent, data.into())
                                            .await
                                        {
                                            tracing::warn!(
                                                ?err,
                                                "error processing incoming broadcast"
                                            );
                                        }
                                    }
                                    wire::Wire::Gossip(wire::Gossip {
                                        space,
                                        data,
                                        module,
                                    }) => {
                                        let data: Vec<u8> = data.into();
                                        let data: Box<[u8]> = data.into_boxed_slice();
                                        if let Err(e) =
                                            i_s.incoming_gossip(space, con, url, data, module).await
                                        {
                                            tracing::warn!(
                                                "failed to handle incoming gossip: {:?}",
                                                e
                                            );
                                        }
                                    }
                                    data => unimplemented!("{:?}", data),
                                }
                            }
                            _ => (),
                        }
                    }
                })
                .await;
                tracing::warn!("KitsuneP2p tx2:ep poll shutdown");
            }
        });

        let bandwidth_throttles = BandwidthThrottles::new(&config.tuning_params);
        let parallel_notify_permit = Arc::new(tokio::sync::Semaphore::new(
            config.tuning_params.concurrent_limit_per_thread,
        ));

        Ok(Self {
            this_addr: this_addr.into(),
            channel_factory,
            internal_sender,
            evt_sender,
            ep_hnd,
            spaces: HashMap::new(),
            config: Arc::new(config),
            bandwidth_throttles,
            parallel_notify_permit,
        })
    }
}

use ghost_actor::dependencies::must_future::MustBoxFuture;
impl ghost_actor::GhostControlHandler for KitsuneP2pActor {
    fn handle_ghost_actor_shutdown(mut self) -> MustBoxFuture<'static, ()> {
        use futures::sink::SinkExt;
        use ghost_actor::GhostControlSender;
        async move {
            // this is a curtesy, ok if fails
            let _ = self.evt_sender.close().await;
            self.ep_hnd.close(500, "").await;
            for (_, space) in self.spaces.into_iter() {
                let (space, _) = space.get().await;
                let _ = space.ghost_actor_shutdown_immediate().await;
            }
        }
        .boxed()
        .into()
    }
}

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

    fn handle_incoming_delegate_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        basis: Arc<KitsuneBasis>,
        to_agent: Arc<KitsuneAgent>,
        mod_idx: u32,
        mod_cnt: u32,
        data: crate::wire::WireData,
    ) -> InternalHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => {
                tracing::warn!(
                    "received delegate_broadcast for unhandled space: {:?}",
                    space
                );
                return unit_ok_fut();
            }
            Some(space) => space.get(),
        };
        Ok(async move {
            let (_, space_inner) = space_sender.await;
            space_inner
                .incoming_delegate_broadcast(space, basis, to_agent, mod_idx, mod_cnt, data)
                .await
        }
        .boxed()
        .into())
    }

    fn handle_incoming_gossip(
        &mut self,
        space: Arc<KitsuneSpace>,
        con: Tx2ConHnd<wire::Wire>,
        remote_url: TxUrl,
        data: Box<[u8]>,
        module_type: GossipModuleType,
    ) -> InternalHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => {
                tracing::warn!("received gossip for unhandled space: {:?}", space);
                return unit_ok_fut();
            }
            Some(space) => space.get(),
        };
        Ok(async move {
            let (_, space_inner) = space_sender.await;
            space_inner
                .incoming_gossip(space, con, remote_url, data, module_type)
                .await
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for KitsuneP2pActor {}

impl KitsuneP2pEventHandler for KitsuneP2pActor {
    fn handle_put_agent_info_signed(
        &mut self,
        input: crate::event::PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.put_agent_info_signed(input))
    }

    fn handle_get_agent_info_signed(
        &mut self,
        input: crate::event::GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        Ok(self.evt_sender.get_agent_info_signed(input))
    }

    fn handle_query_agents(
        &mut self,
        input: crate::event::QueryAgentsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>> {
        Ok(self.evt_sender.query_agents(input))
    }

    fn handle_query_peer_density(
        &mut self,
        space: Arc<KitsuneSpace>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht_arc::PeerDensity> {
        Ok(self.evt_sender.query_peer_density(space, dht_arc))
    }

    fn handle_put_metric_datum(&mut self, datum: MetricDatum) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.put_metric_datum(datum))
    }

    fn handle_query_metrics(
        &mut self,
        query: MetricQuery,
    ) -> KitsuneP2pEventHandlerResult<MetricQueryAnswer> {
        Ok(self.evt_sender.query_metrics(query))
    }

    fn handle_call(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        Ok(self.evt_sender.call(space, to_agent, from_agent, payload))
    }

    fn handle_notify(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.notify(space, to_agent, from_agent, payload))
    }

    fn handle_gossip(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.gossip(space, to_agent, ops))
    }

    fn handle_fetch_op_data(
        &mut self,
        input: FetchOpDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
        Ok(self.evt_sender.fetch_op_data(input))
    }

    fn handle_query_op_hashes(
        &mut self,
        input: QueryOpHashesEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)>> {
        Ok(self.evt_sender.query_op_hashes(input))
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
    fn handle_list_transport_bindings(&mut self) -> KitsuneP2pHandlerResult<Vec<url2::Url2>> {
        let this_addr = vec![self.this_addr.clone()];
        Ok(async move { Ok(this_addr) }.boxed().into())
    }

    fn handle_join(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let internal_sender = self.internal_sender.clone();
        let space2 = space.clone();
        let this_addr = self.this_addr.clone();
        let ep_hnd = self.ep_hnd.clone();
        let config = Arc::clone(&self.config);
        let bandwidth_throttles = self.bandwidth_throttles.clone();
        let parallel_notify_permit = self.parallel_notify_permit.clone();
        let space_sender = match self.spaces.entry(space.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(AsyncLazy::new(async move {
                let (send, send_inner, evt_recv) = spawn_space(
                    space2,
                    this_addr,
                    ep_hnd,
                    config,
                    bandwidth_throttles,
                    parallel_notify_permit,
                )
                .await
                .expect("cannot fail to create space");
                internal_sender
                    .register_space_event_handler(evt_recv)
                    .await
                    .expect("FAIL");
                (send, send_inner)
            })),
        };
        let space_sender = space_sender.get();
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender.join(space, agent).await
        }
        .boxed()
        .into())
    }

    fn handle_leave(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return unit_ok_fut(),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender.leave(space.clone(), agent).await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_rpc_single(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
        timeout_ms: Option<u64>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender
                .rpc_single(space, to_agent, from_agent, payload, timeout_ms)
                .await
        }
        .boxed()
        .into())
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
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender.rpc_multi(input).await
        }
        .boxed()
        .into())
    }

    fn handle_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        basis: Arc<KitsuneBasis>,
        timeout: KitsuneTimeout,
        payload: Vec<u8>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender.broadcast(space, basis, timeout, payload).await
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
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender
                .targeted_broadcast(space, from_agent, agents, timeout, payload)
                .await
        }
        .boxed()
        .into())
    }

    fn handle_new_integrated_data(
        &mut self,
        space: Arc<KitsuneSpace>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return unit_ok_fut(),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender.new_integrated_data(space).await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_authority_for_hash(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        basis: Arc<KitsuneBasis>,
    ) -> KitsuneP2pHandlerResult<bool> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender.authority_for_hash(space, agent, basis).await
        }
        .boxed()
        .into())
    }
}

#[cfg(test)]
mockall::mock! {

    pub KitsuneP2pEventHandler {}

    impl KitsuneP2pEventHandler for KitsuneP2pEventHandler {
        fn handle_put_agent_info_signed(
            &mut self,
            input: crate::event::PutAgentInfoSignedEvt,
        ) -> KitsuneP2pEventHandlerResult<()>;

        fn handle_get_agent_info_signed(
            &mut self,
            input: crate::event::GetAgentInfoSignedEvt,
        ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>>;

        fn handle_query_agents(
            &mut self,
            input: crate::event::QueryAgentsEvt,
        ) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>>;

        fn handle_put_metric_datum(&mut self, datum: MetricDatum) -> KitsuneP2pEventHandlerResult<()>;

        fn handle_query_metrics(
            &mut self,
            query: MetricQuery,
        ) -> KitsuneP2pEventHandlerResult<MetricQueryAnswer>;

        fn handle_query_peer_density(
            &mut self,
            space: Arc<KitsuneSpace>,
            dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
        ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht_arc::PeerDensity>;

        fn handle_call(
            &mut self,
            space: Arc<KitsuneSpace>,
            to_agent: Arc<KitsuneAgent>,
            from_agent: Arc<KitsuneAgent>,
            payload: Vec<u8>,
        ) -> KitsuneP2pEventHandlerResult<Vec<u8>>;

        fn handle_notify(
            &mut self,
            space: Arc<KitsuneSpace>,
            to_agent: Arc<KitsuneAgent>,
            from_agent: Arc<KitsuneAgent>,
            payload: Vec<u8>,
        ) -> KitsuneP2pEventHandlerResult<()> ;

        fn handle_gossip(
            &mut self,
            space: Arc<KitsuneSpace>,
            to_agent: Arc<KitsuneAgent>,
            ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
        ) -> KitsuneP2pEventHandlerResult<()>;

        fn handle_query_op_hashes(
            &mut self,
            input: QueryOpHashesEvt,
        ) -> KitsuneP2pEventHandlerResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)>>;

        fn handle_fetch_op_data(
            &mut self,
            input: FetchOpDataEvt,
        ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> ;

        fn handle_sign_network_data(
            &mut self,
            input: SignNetworkDataEvt,
        ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> ;

    }
}

#[cfg(test)]
impl ghost_actor::GhostHandler<KitsuneP2pEvent> for MockKitsuneP2pEventHandler {}
#[cfg(test)]
impl ghost_actor::GhostControlHandler for MockKitsuneP2pEventHandler {}
