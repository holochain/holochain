// this is largely a passthrough that routes to a specific space handler

use crate::actor;
use crate::actor::*;
use crate::event::*;
use crate::gossip::sharded_gossip::BandwidthThrottles;
use crate::gossip::sharded_gossip::KitsuneDiagnostics;
use crate::types::gossip::GossipModuleType;
use crate::types::metrics::KitsuneMetrics;
use crate::wire::MetricExchangeMsg;
use crate::*;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use kitsune_p2p_fetch::*;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::async_lazy::AsyncLazy;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::tx2_restart_adapter::*;
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

/// The bootstrap service is much more thoroughly documented in the default service implementation.
/// See <https://github.com/holochain/bootstrap>
mod bootstrap;
mod discover;
mod space;
use ghost_actor::dependencies::tracing;
use space::*;

type EvtRcv = futures::channel::mpsc::Receiver<KitsuneP2pEvent>;
type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KBasis = Arc<KitsuneBasis>;
type VecMXM = Vec<MetricExchangeMsg>;
type WireConHnd = Tx2ConHnd<wire::Wire>;
type Payload = Box<[u8]>;
type OpHashList = Vec<OpHashSized>;
type MaybeDelegate = Option<(KBasis, u32, u32)>;

ghost_actor::ghost_chan! {
    #[allow(clippy::too_many_arguments)]
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

        /// We just received data for an op_hash. Check if we had a pending
        /// delegation action we need to continue now that we have the data.
        fn resolve_publish_pending_delegates(space: KSpace, op_hash: KOpHash) -> ();

        /// Incoming Gossip
        fn incoming_gossip(space: KSpace, con: WireConHnd, remote_url: kitsune_p2p_types::tx2::tx2_utils::TxUrl, data: Payload, module_type: crate::types::gossip::GossipModuleType) -> ();

        /// Incoming Metric Exchange
        fn incoming_metric_exchange(space: KSpace, msgs: VecMXM) -> ();

        /// New Con
        fn new_con(url: TxUrl, con: WireConHnd) -> ();

        /// Del Con
        fn del_con(url: TxUrl) -> ();

        /// Fetch an op from a remote
        fn fetch(key: FetchKey, space: KSpace, source: FetchSource) -> ();
    }
}

pub(crate) struct KitsuneP2pActor {
    channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
    internal_sender: ghost_actor::GhostSender<Internal>,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    host: HostApi,
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
    fetch_queue: FetchQueue,
}

impl KitsuneP2pActor {
    pub async fn new(
        config: KitsuneP2pConfig,
        tls_config: kitsune_p2p_types::tls::TlsConfig,
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        internal_sender: ghost_actor::GhostSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
        host: HostApi,
    ) -> KitsuneP2pResult<Self> {
        crate::types::metrics::init();

        let tx2_conf = config.to_tx2().map_err(KitsuneP2pError::other)?;

        let mut is_mock = false;

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
            KitsuneP2pTx2Backend::Mock { mock_network } => {
                is_mock = true;
                (mock_network, "none:".into())
            }
        };

        // wrap in restart logic
        let f = tx2_restart_adapter(f);

        // convert to frontend
        let f = tx2_pool_promote(f, config.tuning_params.clone());

        // wrap in proxy
        let f = if !is_mock {
            let mut conf = kitsune_p2p_proxy::tx2::ProxyConfig::default();
            conf.tuning_params = Some(config.tuning_params.clone());
            match tx2_conf.use_proxy {
                KitsuneP2pTx2ProxyConfig::NoProxy => (),
                KitsuneP2pTx2ProxyConfig::Specific(proxy_url) => {
                    conf.client_of_remote_proxy = ProxyRemoteType::Specific(proxy_url);
                }
                KitsuneP2pTx2ProxyConfig::Bootstrap {
                    bootstrap_url,
                    fallback_proxy_url,
                } => {
                    conf.client_of_remote_proxy = ProxyRemoteType::Bootstrap {
                        bootstrap_url,
                        fallback_proxy_url,
                    };
                    conf.proxy_from_bootstrap_cb = Arc::new(|bootstrap_url| {
                        Box::pin(async move {
                            match bootstrap::proxy_list(bootstrap_url.into()).await {
                                Ok(mut proxy_list) => {
                                    if proxy_list.is_empty() {
                                        return None;
                                    }
                                    use rand::Rng;
                                    Some(
                                        proxy_list
                                            .remove(
                                                rand::thread_rng().gen_range(0..proxy_list.len()),
                                            )
                                            .into(),
                                    )
                                }
                                _ => None,
                            }
                        })
                    });
                }
            }
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

        struct FetchResponseConfig(kitsune_p2p_types::config::KitsuneP2pTuningParams);

        impl kitsune_p2p_fetch::FetchResponseConfig for FetchResponseConfig {
            type User = (
                Tx2ConHnd<wire::Wire>,
                TxUrl,
                Option<(dht::prelude::RegionCoords, bool)>,
            );

            fn respond(
                &self,
                space: KSpace,
                user: Self::User,
                completion_guard: kitsune_p2p_fetch::FetchResponseGuard,
                op: KOpData,
            ) {
                let timeout = self.0.implicit_timeout();
                tokio::task::spawn(async move {
                    let _completion_guard = completion_guard;

                    // MAYBE: open a new connection if the con was closed??
                    let (con, _url, region) = user;

                    let item = wire::PushOpItem {
                        op_data: op,
                        region,
                    };
                    tracing::debug!("push_op_data: {:?}", item);
                    let payload = wire::Wire::push_op_data(vec![(space, vec![item])]);

                    if let Err(err) = con.notify(&payload, timeout).await {
                        tracing::warn!(?err, "error responding to op fetch");
                    }
                });
            }
        }

        let fetch_response_queue = kitsune_p2p_fetch::FetchResponseQueue::new(FetchResponseConfig(
            config.tuning_params.clone(),
        ));

        // TODO - use a real config
        let fetch_queue = FetchQueue::new_bitwise_or();

        // Start a loop to handle our fetch queue fetch items.
        {
            let fetch_queue = fetch_queue.clone();
            let i_s = internal_sender.clone();
            let host = host.clone();
            tokio::task::spawn(async move {
                loop {
                    let list = fetch_queue.get_items_to_fetch();

                    for (key, space, source, context) in list {
                        if let FetchKey::Op(op_hash) = &key {
                            if let Ok(mut res) = host
                                .check_op_data(space.clone(), vec![op_hash.clone()], context)
                                .await
                            {
                                if res.len() == 1 && res.remove(0) {
                                    fetch_queue.remove(&key);
                                    continue;
                                }
                            }
                        }

                        if let Err(err) = i_s.fetch(key, space, source).await {
                            tracing::debug!(?err);
                        }
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            });
        }

        let i_s = internal_sender.clone();
        tokio::task::spawn({
            let evt_sender = evt_sender.clone();
            let host = host.clone();
            let tuning_params = config.tuning_params.clone();
            let fetch_queue = fetch_queue.clone();
            async move {
                let fetch_response_queue = &fetch_response_queue;
                let fetch_queue = &fetch_queue;
                ep.for_each_concurrent(tuning_params.concurrent_limit_per_thread, move |event| {
                    let evt_sender = evt_sender.clone();
                    let host = host.clone();
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
                            OutgoingConnection(Tx2EpConnection { con, url }) => {
                                let _ = i_s.new_con(url, con).await;
                            }
                            IncomingConnection(Tx2EpConnection { con, url }) => {
                                let _ = i_s.new_con(url, con).await;
                            }
                            ConnectionClosed(Tx2EpConnectionClosed { url, .. }) => {
                                let _ = i_s.del_con(url).await;
                            }
                            IncomingRequest(Tx2EpIncomingRequest { data, respond, .. }) => {
                                match data {
                                    wire::Wire::Call(wire::Call {
                                        space,
                                        to_agent,
                                        data,
                                        ..
                                    }) => {
                                        let res = match evt_sender
                                            .call(space, to_agent, data.into())
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
                                        if let Ok(Some(agent_info_signed)) = host
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
                                    }) => match data {
                                        BroadcastData::Publish {
                                            source,
                                            op_hash_list,
                                            context,
                                        } => {
                                            if let Err(err) = i_s
                                                .incoming_publish(
                                                    space,
                                                    to_agent,
                                                    source,
                                                    op_hash_list,
                                                    context,
                                                    Some((basis, mod_idx, mod_cnt)),
                                                )
                                                .await
                                            {
                                                tracing::warn!(
                                                    ?err,
                                                    "failed to handle incoming delegate broadcast"
                                                );
                                            }
                                        }
                                        data => {
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
                                    },
                                    wire::Wire::Broadcast(wire::Broadcast {
                                        space,
                                        to_agent,
                                        data,
                                        ..
                                    }) => match data {
                                        BroadcastData::User(data) => {
                                            // TODO: Should we check if the basis is
                                            // held before calling notify?
                                            if let Err(err) =
                                                evt_sender.notify(space, to_agent, data).await
                                            {
                                                tracing::warn!(
                                                    ?err,
                                                    "error processing incoming broadcast"
                                                );
                                            }
                                        }
                                        BroadcastData::AgentInfo(agent_info) => {
                                            // TODO: Should we check if the basis is
                                            // held before calling put_agent_info_signed?
                                            if let Err(err) = evt_sender
                                                .put_agent_info_signed(PutAgentInfoSignedEvt {
                                                    space,
                                                    peer_data: vec![agent_info],
                                                })
                                                .await
                                            {
                                                tracing::warn!(
                                                    ?err,
                                                    "error processing incoming agent info broadcast"
                                                );
                                            }
                                        }
                                        BroadcastData::Publish {
                                            source,
                                            op_hash_list,
                                            context,
                                        } => {
                                            if let Err(err) = i_s
                                                .incoming_publish(
                                                    space,
                                                    to_agent,
                                                    source,
                                                    op_hash_list,
                                                    context,
                                                    None,
                                                )
                                                .await
                                            {
                                                tracing::warn!(
                                                    ?err,
                                                    "failed to handle incoming broadcast"
                                                );
                                            }
                                        }
                                    },
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
                                    wire::Wire::FetchOp(wire::FetchOp { fetch_list }) => {
                                        for (space, key_list) in fetch_list {
                                            let mut hashes = Vec::new();
                                            let topo = match host.get_topology(space.clone()).await
                                            {
                                                Err(_) => continue,
                                                Ok(topo) => topo,
                                            };
                                            let mut regions = Vec::new();

                                            for key in key_list {
                                                match key {
                                                    FetchKey::Region(region_coords) => {
                                                        regions.push((
                                                            region_coords,
                                                            region_coords.to_bounds(&topo),
                                                        ));
                                                    }
                                                    FetchKey::Op(op_hash) => {
                                                        hashes.push(op_hash);
                                                    }
                                                }
                                            }

                                            if !hashes.is_empty() {
                                                //let mut found = std::collections::HashMap::new();
                                                //for hash in hashes.iter() {
                                                //    found.insert(hash.clone(), false);
                                                //}
                                                if let Ok(list) = evt_sender
                                                    .fetch_op_data(FetchOpDataEvt {
                                                        space: space.clone(),
                                                        query: FetchOpDataEvtQuery::Hashes {
                                                            op_hash_list: hashes,
                                                            include_limbo: true,
                                                        },
                                                    })
                                                    .await
                                                {
                                                    for (_hash, op) in list {
                                                        //found.insert(hash, true);
                                                        fetch_response_queue.enqueue_op(
                                                            space.clone(),
                                                            (con.clone(), url.clone(), None),
                                                            op,
                                                        );
                                                    }
                                                }
                                                //tracing::warn!(?found, "fetch op data responder");
                                            }

                                            for (coord, bound) in regions {
                                                if let Ok(list) = evt_sender
                                                    .fetch_op_data(FetchOpDataEvt {
                                                        space: space.clone(),
                                                        query: FetchOpDataEvtQuery::Regions(vec![
                                                            bound,
                                                        ]),
                                                    })
                                                    .await
                                                {
                                                    let last_idx = list.len() - 1;
                                                    for (idx, (_hash, op)) in
                                                        list.into_iter().enumerate()
                                                    {
                                                        fetch_response_queue.enqueue_op(
                                                            space.clone(),
                                                            (
                                                                con.clone(),
                                                                url.clone(),
                                                                Some((coord, idx == last_idx)),
                                                            ),
                                                            op,
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    wire::Wire::PushOpData(wire::PushOpData { op_data_list }) => {
                                        for (space, op_list) in op_data_list {
                                            for op in op_list {
                                                // hash the op
                                                let op_hash =
                                                    match host.op_hash(op.op_data.clone()).await {
                                                        Ok(op_hash) => op_hash,
                                                        Err(_) => continue,
                                                    };

                                                // trigger any delegation
                                                // that is pending on
                                                // having this data
                                                let _ = i_s
                                                    .resolve_publish_pending_delegates(
                                                        space.clone(),
                                                        op_hash.clone(),
                                                    )
                                                    .await;

                                                // MAYBE: do something with the
                                                //        is_last bool?
                                                //        Right now we don't
                                                //        really care, because
                                                //        if it's a region
                                                //        we know it's gossip
                                                //        so it's okay if
                                                //        the context is
                                                //        `None`.
                                                let key =
                                                    if let Some((region, _is_last)) = op.region {
                                                        FetchKey::Region(region)
                                                    } else {
                                                        FetchKey::Op(op_hash.clone())
                                                    };
                                                let fetch_context = fetch_queue
                                                    .remove(&key)
                                                    .and_then(|i| i.context);

                                                // forward the received op
                                                let _ = evt_sender
                                                    .receive_ops(
                                                        space.clone(),
                                                        vec![op.op_data],
                                                        fetch_context,
                                                    )
                                                    .await;
                                            }
                                        }
                                    }
                                    wire::Wire::MetricExchange(wire::MetricExchange {
                                        space,
                                        msgs,
                                    }) => {
                                        let _ = i_s.incoming_metric_exchange(space, msgs).await;
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
            channel_factory,
            internal_sender,
            evt_sender,
            ep_hnd,
            host,
            spaces: HashMap::new(),
            config: Arc::new(config),
            bandwidth_throttles,
            parallel_notify_permit,
            fetch_queue,
        })
    }
}

use ghost_actor::dependencies::must_future::MustBoxFuture;
impl ghost_actor::GhostControlHandler for KitsuneP2pActor {
    fn handle_ghost_actor_shutdown(mut self) -> MustBoxFuture<'static, ()> {
        use futures::sink::SinkExt;
        use ghost_actor::GhostControlSender;
        async move {
            // The line below was added when migrating to rust edition 2021, per
            // https://doc.rust-lang.org/edition-guide/rust-2021/disjoint-capture-in-closures.html#migration
            let _ = &self;
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
        data: BroadcastData,
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

    fn handle_incoming_publish(
        &mut self,
        space: KSpace,
        to_agent: KAgent,
        source: KAgent,
        op_hash_list: OpHashList,
        context: kitsune_p2p_fetch::FetchContext,
        maybe_delegate: MaybeDelegate,
    ) -> InternalHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => {
                tracing::warn!("received publish for unhandled space: {:?}", space);
                return unit_ok_fut();
            }
            Some(space) => space.get(),
        };
        Ok(async move {
            let (_, space_inner) = space_sender.await;
            space_inner
                .incoming_publish(
                    space,
                    to_agent,
                    source,
                    op_hash_list,
                    context,
                    maybe_delegate,
                )
                .await
        }
        .boxed()
        .into())
    }

    fn handle_resolve_publish_pending_delegates(
        &mut self,
        space: KSpace,
        op_hash: KOpHash,
    ) -> InternalHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => {
                return unit_ok_fut();
            }
            Some(space) => space.get(),
        };
        Ok(async move {
            let (_, space_inner) = space_sender.await;
            space_inner
                .resolve_publish_pending_delegates(space, op_hash)
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

    fn handle_incoming_metric_exchange(
        &mut self,
        space: Arc<KitsuneSpace>,
        msgs: Vec<MetricExchangeMsg>,
    ) -> InternalHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => {
                return unit_ok_fut();
            }
            Some(space) => space.get(),
        };
        Ok(async move {
            let (_, space_inner) = space_sender.await;
            space_inner.incoming_metric_exchange(space, msgs).await
        }
        .boxed()
        .into())
    }

    fn handle_new_con(
        &mut self,
        url: TxUrl,
        con: Tx2ConHnd<wire::Wire>,
    ) -> InternalHandlerResult<()> {
        let spaces = self.spaces.iter().map(|(_, s)| s.get()).collect::<Vec<_>>();
        Ok(async move {
            let mut all = Vec::new();
            for (_, space) in futures::future::join_all(spaces).await {
                all.push(space.new_con(url.clone(), con.clone()));
            }
            let _ = futures::future::join_all(all).await;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_del_con(&mut self, url: TxUrl) -> InternalHandlerResult<()> {
        let spaces = self.spaces.iter().map(|(_, s)| s.get()).collect::<Vec<_>>();
        Ok(async move {
            let mut all = Vec::new();
            for (_, space) in futures::future::join_all(spaces).await {
                all.push(space.del_con(url.clone()));
            }
            let _ = futures::future::join_all(all).await;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_fetch(
        &mut self,
        key: FetchKey,
        space: KSpace,
        source: FetchSource,
    ) -> InternalHandlerResult<()> {
        let FetchSource::Agent(agent) = source;

        let space_sender = match self.spaces.get_mut(&space) {
            None => {
                tracing::warn!("received fetch for unhandled space: {:?}", space);
                return unit_ok_fut();
            }
            Some(space) => space.get(),
        };
        Ok(async move {
            let (_, space_inner) = space_sender.await;
            let payload = wire::Wire::fetch_op(vec![(space, vec![key])]);
            space_inner.notify(agent, payload).await
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
    ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht::PeerView> {
        Ok(self.evt_sender.query_peer_density(space, dht_arc))
    }

    fn handle_call(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        Ok(self.evt_sender.call(space, to_agent, payload))
    }

    fn handle_notify(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.notify(space, to_agent, payload))
    }

    fn handle_receive_ops(
        &mut self,
        space: Arc<KitsuneSpace>,
        ops: Vec<KOp>,
        context: Option<FetchContext>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.receive_ops(space, ops, context))
    }

    fn handle_fetch_op_data(
        &mut self,
        input: FetchOpDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, KOp)>> {
        Ok(self.evt_sender.fetch_op_data(input))
    }

    fn handle_query_op_hashes(
        &mut self,
        input: QueryOpHashesEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindowInclusive)>> {
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
        let this_addr = self.ep_hnd.local_addr();
        Ok(async move { Ok(vec![this_addr?.into()]) }.boxed().into())
    }

    fn handle_join(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        initial_arc: Option<crate::dht_arc::DhtArc>,
    ) -> KitsuneP2pHandlerResult<()> {
        let internal_sender = self.internal_sender.clone();
        let space2 = space.clone();
        let ep_hnd = self.ep_hnd.clone();
        let host = self.host.clone();
        let config = Arc::clone(&self.config);
        let bandwidth_throttles = self.bandwidth_throttles.clone();
        let parallel_notify_permit = self.parallel_notify_permit.clone();
        let fetch_queue = self.fetch_queue.clone();

        let space_sender = match self.spaces.entry(space.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(AsyncLazy::new(async move {
                let (send, send_inner, evt_recv) = spawn_space(
                    space2,
                    ep_hnd,
                    host,
                    config,
                    bandwidth_throttles,
                    parallel_notify_permit,
                    fetch_queue,
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
            space_sender.join(space, agent, initial_arc).await
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
                .rpc_single(space, to_agent, payload, timeout_ms)
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
        data: BroadcastData,
    ) -> KitsuneP2pHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender.broadcast(space, basis, timeout, data).await
        }
        .boxed()
        .into())
    }

    fn handle_targeted_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        agents: Vec<Arc<KitsuneAgent>>,
        timeout: KitsuneTimeout,
        payload: Vec<u8>,
        drop_at_limit: bool,
    ) -> KitsuneP2pHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender
                .targeted_broadcast(space, agents, timeout, payload, drop_at_limit)
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
        basis: Arc<KitsuneBasis>,
    ) -> KitsuneP2pHandlerResult<bool> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender.authority_for_hash(space, basis).await
        }
        .boxed()
        .into())
    }

    fn handle_dump_network_metrics(
        &mut self,
        space: Option<Arc<KitsuneSpace>>,
    ) -> KitsuneP2pHandlerResult<serde_json::Value> {
        let spaces = self
            .spaces
            .iter()
            .filter_map(|(h, s)| {
                if let Some(space) = &space {
                    if h != space {
                        return None;
                    }
                }
                let h = h.clone();
                let s = s.get();
                Some(s.then(move |r| async move { (h, r) }))
            })
            .collect::<Vec<_>>();
        Ok(async move {
            let mut all = Vec::new();
            for (h, (space, _)) in futures::future::join_all(spaces).await {
                all.push(space.dump_network_metrics(Some(h)));
            }
            Ok(futures::future::try_join_all(all).await?.into())
        }
        .boxed()
        .into())
    }

    fn handle_get_diagnostics(
        &mut self,
        space: KSpace,
        // gossip_type: GossipModuleType,
    ) -> KitsuneP2pHandlerResult<KitsuneDiagnostics> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            let (space_sender, _) = space_sender.await;
            space_sender.get_diagnostics(space).await
        }
        .boxed()
        .into())
    }
}

#[cfg(any(test, feature = "test_utils"))]
mockall::mock! {

    pub KitsuneP2pEventHandler {}

    impl KitsuneP2pEventHandler for KitsuneP2pEventHandler {

        fn handle_put_agent_info_signed(
            &mut self,
            input: crate::event::PutAgentInfoSignedEvt,
        ) -> KitsuneP2pEventHandlerResult<()>;

        fn handle_query_agents(
            &mut self,
            input: crate::event::QueryAgentsEvt,
        ) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>>;

        fn handle_query_peer_density(
            &mut self,
            space: Arc<KitsuneSpace>,
            dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
        ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht::PeerView>;

        fn handle_call(
            &mut self,
            space: Arc<KitsuneSpace>,
            to_agent: Arc<KitsuneAgent>,
            payload: Vec<u8>,
        ) -> KitsuneP2pEventHandlerResult<Vec<u8>>;

        fn handle_notify(
            &mut self,
            space: Arc<KitsuneSpace>,
            to_agent: Arc<KitsuneAgent>,
            payload: Vec<u8>,
        ) -> KitsuneP2pEventHandlerResult<()> ;

        fn handle_receive_ops(
            &mut self,
            space: Arc<KitsuneSpace>,
            ops: Vec<KOp>,
            context: Option<FetchContext>,
        ) -> KitsuneP2pEventHandlerResult<()>;

        fn handle_query_op_hashes(
            &mut self,
            input: QueryOpHashesEvt,
        ) -> KitsuneP2pEventHandlerResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindowInclusive)>>;

        fn handle_fetch_op_data(
            &mut self,
            input: FetchOpDataEvt,
        ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, KOp)>> ;

        fn handle_sign_network_data(
            &mut self,
            input: SignNetworkDataEvt,
        ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> ;

    }
}

#[cfg(any(test, feature = "test_utils"))]
impl ghost_actor::GhostHandler<KitsuneP2pEvent> for MockKitsuneP2pEventHandler {}
#[cfg(any(test, feature = "test_utils"))]
impl ghost_actor::GhostControlHandler for MockKitsuneP2pEventHandler {}
