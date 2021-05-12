// this is largely a passthrough that routes to a specific space handler

use crate::actor;
use crate::actor::*;
use crate::event::*;
use crate::gossip::*;
use crate::metrics::KitsuneMetrics;
use crate::*;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_proxy::ProxyUrl;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::async_lazy::AsyncLazy;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

/// The bootstrap service is much more thoroughly documented in the default service implementation.
/// See https://github.com/holochain/bootstrap
mod bootstrap;
mod discover;
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
    this_addr: url2::Url2,
    channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
    internal_sender: ghost_actor::GhostSender<Internal>,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    spaces: HashMap<Arc<KitsuneSpace>, AsyncLazy<ghost_actor::GhostSender<KitsuneP2p>>>,
    config: Arc<KitsuneP2pConfig>,
}

impl KitsuneP2pActor {
    pub async fn new(
        config: KitsuneP2pConfig,
        tls_config: kitsune_p2p_proxy::TlsConfig,
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        internal_sender: ghost_actor::GhostSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) -> KitsuneP2pResult<Self> {
        crate::types::metrics::init();

        let tx2_conf = config.to_tx2().map_err(KitsuneP2pError::other)?;

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
        };

        // convert to frontend
        let f = tx2_pool_promote(f, config.tuning_params.clone());

        // wrap in proxy
        let mut conf = kitsune_p2p_proxy::tx2::ProxyConfig::default();
        conf.tuning_params = Some(config.tuning_params.clone());
        let f = tx2_proxy(f, conf)?;

        let metrics = Tx2ApiMetrics::default().set_write_len(|d, l| {
            let t = match d {
                "Wire::Failure" => KitsuneMetrics::Fail,
                "Wire::Call" => KitsuneMetrics::Call,
                "Wire::CallResp" => KitsuneMetrics::CallResp,
                "Wire::Notify" => KitsuneMetrics::Notify,
                "Wire::NotifyResp" => KitsuneMetrics::NotifyResp,
                "Wire::FetchOpHashes" => KitsuneMetrics::FetchOpHashes,
                "Wire::FetchOpHashesResponse" => KitsuneMetrics::FetchOpHashesResp,
                "Wire::FetchOpData" => KitsuneMetrics::FetchOpData,
                "Wire::FetchOpDataResponse" => KitsuneMetrics::FetchOpDataResp,
                "Wire::AgentInfoQuery" => KitsuneMetrics::AgentInfoQuery,
                "Wire::AgentInfoQueryResp" => KitsuneMetrics::AgentInfoQueryResp,
                "Wire::Gossip" => KitsuneMetrics::Gossip,
                "Wire::GossipResp" => KitsuneMetrics::GossipResp,
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
            });

            ProxyUrl::new(proxy_url.as_base().as_str(), this_digest)
                .unwrap()
                .as_str()
                .into()
        } else {
            ep_hnd.local_addr().map_err(KitsuneP2pError::other)?
        };

        tracing::info!("this_addr: {}", this_addr);

        tokio::task::spawn({
            let evt_sender = evt_sender.clone();
            let tuning_params = config.tuning_params.clone();
            ep.for_each_concurrent(tuning_params.concurrent_limit_per_thread, move |event| {
                let evt_sender = evt_sender.clone();
                let tuning_params = tuning_params.clone();
                async move {
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
                                            let _ = respond
                                                .respond(fail, tuning_params.implicit_timeout())
                                                .await;
                                            return;
                                        }
                                        Ok(r) => r,
                                    };
                                    let resp = wire::Wire::call_resp(res.into());
                                    let _ = respond
                                        .respond(resp, tuning_params.implicit_timeout())
                                        .await;
                                }
                                wire::Wire::Notify(wire::Notify {
                                    space,
                                    from_agent,
                                    to_agent,
                                    data,
                                    ..
                                }) => {
                                    if let Err(err) = evt_sender
                                        .notify(space, to_agent, from_agent, data.into())
                                        .await
                                    {
                                        let reason = format!("{:?}", err);
                                        let fail = wire::Wire::failure(reason);
                                        let _ = respond
                                            .respond(fail, tuning_params.implicit_timeout())
                                            .await;
                                        return;
                                    }
                                    let resp = wire::Wire::notify_resp();
                                    let _ = respond
                                        .respond(resp, tuning_params.implicit_timeout())
                                        .await;
                                }
                                wire::Wire::FetchOpHashes(wire::FetchOpHashes {
                                    space,
                                    from_agent,
                                    to_agent,
                                    dht_arc,
                                    since_utc_epoch_s,
                                    until_utc_epoch_s,
                                    last_count,
                                }) => {
                                    let input = ReqOpHashesEvt::new(
                                        from_agent,
                                        to_agent,
                                        dht_arc,
                                        since_utc_epoch_s,
                                        until_utc_epoch_s,
                                        Default::default(),
                                    );
                                    let (hashes, agent_hashes) = match local_req_op_hashes(
                                        &evt_sender,
                                        space,
                                        input,
                                    )
                                    .await
                                    {
                                        Err(err) => {
                                            let reason = format!("{:?}", err);
                                            let fail = wire::Wire::failure(reason);
                                            let _ = respond
                                                .respond(fail, tuning_params.implicit_timeout())
                                                .await;
                                            return;
                                        }
                                        Ok(r) => r,
                                    };
                                    let hashes = match last_count {
                                        OpCount::Consistent(last_count) => {
                                            // Requester is consistent,
                                            // now check if we are consistent.
                                            if last_count == hashes.len() as u64 {
                                                OpConsistency::Consistent
                                            } else {
                                                OpConsistency::Variance(hashes)
                                            }
                                        }
                                        // Requester has a variance so we must return hashes.
                                        OpCount::Variance => OpConsistency::Variance(hashes),
                                    };
                                    let resp =
                                        wire::Wire::fetch_op_hashes_response(hashes, agent_hashes);
                                    let _ = respond
                                        .respond(resp, tuning_params.implicit_timeout())
                                        .await;
                                }
                                wire::Wire::FetchOpData(wire::FetchOpData {
                                    space,
                                    from_agent,
                                    to_agent,
                                    op_hashes,
                                    peer_hashes,
                                }) => {
                                    let input = ReqOpDataEvt::new(
                                        from_agent,
                                        to_agent,
                                        op_hashes,
                                        peer_hashes,
                                    );
                                    let (op_data, agent_infos) =
                                        match local_req_op_data(&evt_sender, space, input).await {
                                            Err(err) => {
                                                let reason = format!("{:?}", err);
                                                let fail = wire::Wire::failure(reason);
                                                let _ = respond
                                                    .respond(fail, tuning_params.implicit_timeout())
                                                    .await;
                                                return;
                                            }
                                            Ok(r) => r,
                                        };
                                    let op_data =
                                        op_data.into_iter().map(|(h, op)| (h, op.into())).collect();
                                    let resp =
                                        wire::Wire::fetch_op_data_response(op_data, agent_infos);
                                    let _ = respond
                                        .respond(resp, tuning_params.implicit_timeout())
                                        .await;
                                }
                                wire::Wire::AgentInfoQuery(q) => {
                                    match agent_info_query(q, evt_sender.clone()).await {
                                        Ok(r) => {
                                            let resp = wire::Wire::agent_info_query_resp(r);
                                            let _ = respond
                                                .respond(resp, tuning_params.implicit_timeout())
                                                .await;
                                        }
                                        Err(err) => {
                                            let reason = format!("{:?}", err);
                                            let fail = wire::Wire::failure(reason);
                                            let _ = respond
                                                .respond(fail, tuning_params.implicit_timeout())
                                                .await;
                                        }
                                    }
                                }
                                wire::Wire::Gossip(wire::Gossip {
                                    space,
                                    from_agent,
                                    to_agent,
                                    ops,
                                    agents,
                                }) => {
                                    let input = GossipEvt::new(
                                        from_agent,
                                        to_agent,
                                        ops.into_iter().map(|(k, v)| (k, v.into())).collect(),
                                        agents,
                                    );
                                    if let Err(err) = local_gossip_ops(
                                        tuning_params.clone(),
                                        &evt_sender,
                                        space,
                                        input,
                                    )
                                    .await
                                    {
                                        let reason = format!("{:?}", err);
                                        tracing::error!("got err: {}", reason);
                                        let fail = wire::Wire::failure(reason);
                                        let _ = respond
                                            .respond(fail, tuning_params.implicit_timeout())
                                            .await;
                                        return;
                                    }
                                    let resp = wire::Wire::gossip_resp();
                                    let _ = respond
                                        .respond(resp, tuning_params.implicit_timeout())
                                        .await;
                                }
                                data => unimplemented!("{:?}", data),
                            }
                        }
                        _ => (),
                    }
                }
            })
        });

        Ok(Self {
            this_addr: this_addr.into(),
            channel_factory,
            internal_sender,
            evt_sender,
            ep_hnd,
            spaces: HashMap::new(),
            config: Arc::new(config),
        })
    }
}

async fn agent_info_query(
    q: wire::AgentInfoQuery,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
) -> Result<Vec<crate::types::agent_store::AgentInfoSigned>, KitsuneP2pError> {
    let wire::AgentInfoQuery {
        space,
        to_agent,
        by_agent,
        by_basis_arc,
    } = q;

    if let Some(by_agent) = by_agent {
        if let Some(agent) = evt_sender
            .get_agent_info_signed(GetAgentInfoSignedEvt {
                space,
                agent: by_agent,
            })
            .await?
        {
            Ok(vec![agent])
        } else {
            Ok(vec![])
        }
    } else if let Some(_by_basis_arc) = by_basis_arc {
        Ok(evt_sender
            .query_agent_info_signed(QueryAgentInfoSignedEvt {
                space,
                agent: to_agent,
            })
            .await?)
    } else {
        Err("must specify by_agent or by_basis_arc".into())
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

    fn handle_query_agent_info_signed(
        &mut self,
        input: crate::event::QueryAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>> {
        Ok(self.evt_sender.query_agent_info_signed(input))
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
        from_agent: Arc<KitsuneAgent>,
        op_hash: Arc<KitsuneOpHash>,
        op_data: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self
            .evt_sender
            .gossip(space, to_agent, from_agent, op_hash, op_data))
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
        let space_sender = match self.spaces.entry(space.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(AsyncLazy::new(async move {
                let (send, evt_recv) = spawn_space(space2, this_addr, ep_hnd, config)
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
            space_sender
                .await
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
