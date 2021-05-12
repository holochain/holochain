use super::*;
use ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::tracing_futures::Instrument;
use kitsune_p2p_mdns::*;
use kitsune_p2p_types::codec::{rmp_decode, rmp_encode};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::sync::atomic::AtomicBool;

/// if the user specifies None or zero (0) for race_timeout_ms
/// (david.b) this is not currently used
const DEFAULT_RPC_MULTI_RACE_TIMEOUT_MS: u64 = 200;

ghost_actor::ghost_chan! {
    pub(crate) chan SpaceInternal<crate::KitsuneP2pError> {
        /// List online agents that claim to be covering a basis hash
        fn list_online_agents_for_basis_hash(space: Arc<KitsuneSpace>, from_agent: Arc<KitsuneAgent>, basis: Arc<KitsuneBasis>) -> HashSet<Arc<KitsuneAgent>>;

        /// Update / publish our agent info
        fn update_agent_info() -> ();

        /// see if an agent is locally joined
        fn is_agent_local(agent: Arc<KitsuneAgent>) -> bool;
    }
}

pub(crate) async fn spawn_space(
    space: Arc<KitsuneSpace>,
    this_addr: url2::Url2,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    config: Arc<KitsuneP2pConfig>,
) -> KitsuneP2pResult<(
    ghost_actor::GhostSender<KitsuneP2p>,
    KitsuneP2pEventReceiver,
)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    // initialize gossip module
    let gossip_recv = gossip::spawn_gossip_module(config.clone());
    builder
        .channel_factory()
        .attach_receiver(gossip_recv)
        .await?;

    let i_s = builder
        .channel_factory()
        .create_channel::<SpaceInternal>()
        .await?;

    let sender = builder
        .channel_factory()
        .create_channel::<KitsuneP2p>()
        .await?;

    tokio::task::spawn(builder.spawn(Space::new(space, this_addr, i_s, evt_send, ep_hnd, config)));

    Ok((sender, evt_recv))
}

impl ghost_actor::GhostHandler<gossip::GossipEvent> for Space {}

impl gossip::GossipEventHandler for Space {
    fn handle_list_neighbor_agents(
        &mut self,
    ) -> gossip::GossipEventHandlerResult<ListNeighborAgents> {
        // while full-sync this is just a clone of list_by_basis
        let local_agents = self
            .local_joined_agents
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let agent = self.local_joined_agents.iter().next().cloned();
        let fut = match agent {
            Some(agent) => self
                .evt_sender
                .query_agent_info_signed(QueryAgentInfoSignedEvt {
                    space: self.space.clone(),
                    agent,
                }),
            None => async { Ok(Vec::new()) }.boxed().into(),
        };
        Ok(async move {
            let remote_agents = fut
                .await?
                .into_iter()
                .map(|ai| Arc::new(ai.into_agent()))
                .filter(|a| !local_agents.contains(a))
                .collect::<Vec<_>>();
            let local_agents = local_agents.into_iter().collect::<Vec<_>>();
            Ok((local_agents, remote_agents))
        }
        .boxed()
        .into())
    }

    fn handle_req_op_hashes(
        &mut self,
        input: ReqOpHashesEvt,
    ) -> gossip::GossipEventHandlerResult<OpHashesAgentHashes> {
        if self.local_joined_agents.contains(&input.to_agent) {
            let fut = local_req_op_hashes(&self.evt_sender, self.space.clone(), input);
            Ok(
                async move { fut.await.map(|r| (OpConsistency::Variance(r.0), r.1)) }
                    .boxed()
                    .into(),
            )
        } else {
            let ReqOpHashesEvt {
                to_agent,
                dht_arc,
                since_utc_epoch_s,
                until_utc_epoch_s,
                from_agent,
                op_count,
            } = input;
            let ep_hnd = self.ep_hnd.clone();
            let evt_sender = self.evt_sender.clone();
            let space = self.space.clone();
            let timeout = self.config.tuning_params.implicit_timeout();
            Ok(async move {
                // see if we have an entry for this agent in our agent_store
                let info = match evt_sender
                    .get_agent_info_signed(GetAgentInfoSignedEvt {
                        space: space.clone(),
                        agent: to_agent.clone(),
                    })
                    .await?
                {
                    None => return Err(KitsuneP2pError::RoutingAgentError(to_agent)),
                    Some(i) => i,
                };
                let data = wire::Wire::fetch_op_hashes(
                    space,
                    from_agent,
                    to_agent,
                    dht_arc,
                    since_utc_epoch_s,
                    until_utc_epoch_s,
                    op_count,
                );
                let info = types::agent_store::AgentInfo::try_from(&info)?;
                let url = info.as_urls_ref().get(0).unwrap().clone();
                let con_hnd = ep_hnd.get_connection(url, timeout).await?;
                let read = con_hnd.request(&data, timeout).await?;
                match read {
                    wire::Wire::Failure(wire::Failure { reason }) => Err(reason.into()),
                    wire::Wire::FetchOpHashesResponse(wire::FetchOpHashesResponse {
                        hashes,
                        peer_hashes,
                    }) => Ok((hashes, peer_hashes)),
                    _ => unreachable!(),
                }
            }
            .boxed()
            .into())
        }
    }

    fn handle_req_op_data(
        &mut self,
        input: ReqOpDataEvt,
    ) -> gossip::GossipEventHandlerResult<OpDataAgentInfo> {
        if self.local_joined_agents.contains(&input.to_agent) {
            let fut = local_req_op_data(&self.evt_sender, self.space.clone(), input);
            Ok(async move { fut.await }.boxed().into())
        } else {
            let ReqOpDataEvt {
                from_agent,
                to_agent,
                op_hashes,
                peer_hashes,
            } = input;
            let ep_hnd = self.ep_hnd.clone();
            let evt_sender = self.evt_sender.clone();
            let space = self.space.clone();
            let timeout = self.config.tuning_params.implicit_timeout();
            Ok(async move {
                // see if we have an entry for this agent in our agent_store
                let info = match evt_sender
                    .get_agent_info_signed(GetAgentInfoSignedEvt {
                        space: space.clone(),
                        agent: to_agent.clone(),
                    })
                    .await?
                {
                    None => return Err(KitsuneP2pError::RoutingAgentError(to_agent)),
                    Some(i) => i,
                };
                let data =
                    wire::Wire::fetch_op_data(space, from_agent, to_agent, op_hashes, peer_hashes);
                let info = types::agent_store::AgentInfo::try_from(&info)?;
                let url = info.as_urls_ref().get(0).unwrap().clone();
                let con_hnd = ep_hnd.get_connection(url, timeout).await?;
                let read = con_hnd.request(&data, timeout).await?;
                match read {
                    wire::Wire::Failure(wire::Failure { reason }) => Err(reason.into()),
                    wire::Wire::FetchOpDataResponse(wire::FetchOpDataResponse {
                        op_data,
                        agent_infos,
                    }) => Ok((
                        op_data.into_iter().map(|(h, d)| (h, d.into())).collect(),
                        agent_infos,
                    )),
                    _ => unreachable!(),
                }
            }
            .boxed()
            .into())
        }
    }

    fn handle_gossip_ops(&mut self, input: GossipEvt) -> gossip::GossipEventHandlerResult<()> {
        let tuning_params = self.config.tuning_params.clone();
        if self.local_joined_agents.contains(&input.to_agent) {
            let fut = local_gossip_ops(tuning_params, &self.evt_sender, self.space.clone(), input);
            Ok(async move { fut.await }.boxed().into())
        } else {
            let GossipEvt {
                from_agent,
                to_agent,
                ops,
                agents,
            } = input;
            let ep_hnd = self.ep_hnd.clone();
            let evt_sender = self.evt_sender.clone();
            let space = self.space.clone();
            let timeout = self.config.tuning_params.implicit_timeout();
            Ok(async move {
                // see if we have an entry for this agent in our agent_store
                let info = match evt_sender
                    .get_agent_info_signed(GetAgentInfoSignedEvt {
                        space: space.clone(),
                        agent: to_agent.clone(),
                    })
                    .await?
                {
                    None => return Err(KitsuneP2pError::RoutingAgentError(to_agent)),
                    Some(i) => i,
                };
                let data = wire::Wire::gossip(
                    space,
                    from_agent.clone(),
                    to_agent.clone(),
                    ops.into_iter().map(|(k, v)| (k, v.into())).collect(),
                    agents,
                );
                let info = types::agent_store::AgentInfo::try_from(&info)?;
                let url = info.as_urls_ref().get(0).unwrap().clone();
                let con_hnd = ep_hnd.get_connection(url.clone(), timeout).await?;
                let read = con_hnd.request(&data, timeout).await?;
                match read {
                    wire::Wire::Failure(wire::Failure { reason }) => Err(dbg!(reason.into())),
                    wire::Wire::GossipResp(_) => Ok(()),
                    _ => unreachable!(),
                }
            }
            .instrument(tracing::debug_span!("handle_gossip_ops"))
            .boxed()
            .into())
        }
    }
}

pub fn local_req_op_hashes(
    evt_sender: &futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    space: Arc<KitsuneSpace>,
    input: ReqOpHashesEvt,
) -> impl std::future::Future<Output = Result<LocalOpHashesAgentHashes, KitsuneP2pError>> {
    let ReqOpHashesEvt {
        to_agent,
        dht_arc,
        since_utc_epoch_s,
        until_utc_epoch_s,
        ..
    } = input;
    let fut = evt_sender.fetch_op_hashes_for_constraints(FetchOpHashesForConstraintsEvt {
        space: space.clone(),
        agent: to_agent.clone(),
        dht_arc,
        since_utc_epoch_s,
        until_utc_epoch_s,
    });
    let peer_fut = evt_sender.query_agent_info_signed(QueryAgentInfoSignedEvt {
        space,
        agent: to_agent,
    });
    async move {
        let agent_infos = peer_fut.await?;
        let agent_infos = agent_infos
            .into_iter()
            .map(|ai| {
                let ai = types::agent_store::AgentInfo::try_from(&ai)?;
                let time = ai.signed_at_ms();
                Ok((Arc::new(ai.into()), time))
            })
            .collect::<Result<Vec<_>, KitsuneP2pError>>()?;
        Ok((fut.await?, agent_infos))
    }
}

pub fn local_req_op_data(
    evt_sender: &futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    space: Arc<KitsuneSpace>,
    input: ReqOpDataEvt,
) -> impl std::future::Future<Output = Result<OpDataAgentInfo, KitsuneP2pError>> {
    let ReqOpDataEvt {
        to_agent,
        op_hashes,
        peer_hashes,
        ..
    } = input;
    // while full-sync just redirecting to self...
    // but eventually some of these will be outgoing remote requests
    let fut = evt_sender.fetch_op_hash_data(FetchOpHashDataEvt {
        space: space.clone(),
        agent: to_agent.clone(),
        op_hashes,
    });
    let peer_fut = evt_sender.query_agent_info_signed(QueryAgentInfoSignedEvt {
        space,
        agent: to_agent,
    });
    async move {
        let agent_infos = peer_fut.await?;
        let peer_hashes = peer_hashes
            .into_iter()
            .map(|a| (*a).clone())
            .collect::<HashSet<_>>();
        let agent_infos = agent_infos
            .into_iter()
            .filter(|ai| peer_hashes.contains(ai.as_agent_ref()))
            .collect();
        Ok((fut.await?, agent_infos))
    }
}

pub fn local_gossip_ops(
    tuning_params: kitsune_p2p_types::config::KitsuneP2pTuningParams,
    evt_sender: &futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    space: Arc<KitsuneSpace>,
    input: GossipEvt,
) -> impl std::future::Future<Output = Result<(), KitsuneP2pError>> {
    let GossipEvt {
        from_agent,
        to_agent,
        ops,
        agents,
    } = input;
    let all = ops
        .into_iter()
        .map(|(op_hash, op_data)| {
            evt_sender.gossip(
                space.clone(),
                to_agent.clone(),
                from_agent.clone(),
                op_hash,
                op_data,
            )
        })
        .collect::<Vec<_>>();
    let all_agents = agents
        .into_iter()
        .map(|agent_info_signed| {
            evt_sender.put_agent_info_signed(PutAgentInfoSignedEvt {
                space: space.clone(),
                agent: to_agent.clone(),
                agent_info_signed,
            })
        })
        .collect::<Vec<_>>();
    async move {
        let to_agent = &to_agent;
        let from_agent = &from_agent;
        futures::stream::iter(all)
                .for_each_concurrent(tuning_params.concurrent_limit_per_thread, |res| async move {
                    if let Err(e) = res.await {
                        ghost_actor::dependencies::tracing::error!(failed_to_gossip_ops = ?e, ?from_agent, ?to_agent);
                    }
                })
                .await;
        futures::stream::iter(all_agents)
                .for_each_concurrent(tuning_params.concurrent_limit_per_thread, |res| async move {
                    if let Err(e) = res.await {
                        ghost_actor::dependencies::tracing::error!(failed_to_gossip_peer_info = ?e, ?from_agent, ?to_agent);
                    }
                })
                .await;
        Ok(())
    }
}

impl ghost_actor::GhostHandler<SpaceInternal> for Space {}

impl SpaceInternalHandler for Space {
    fn handle_list_online_agents_for_basis_hash(
        &mut self,
        _space: Arc<KitsuneSpace>,
        from_agent: Arc<KitsuneAgent>,
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
                agent: from_agent,
            });
        Ok(async move {
            for peer in all_peers_fut.await? {
                res.insert(Arc::new(peer.as_agent_ref().clone()));
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
        let agent_list: Vec<Arc<KitsuneAgent>> = self.local_joined_agents.iter().cloned().collect();
        let bound_url = self.this_addr.clone();
        let evt_sender = self.evt_sender.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        let expires_after = self.config.tuning_params.agent_info_expires_after_ms as u64;
        Ok(async move {
            let urls = vec![bound_url];
            for agent in agent_list {
                let agent_info = crate::types::agent_store::AgentInfo::new(
                    (*space).clone(),
                    (*agent).clone(),
                    urls.clone(),
                    crate::spawn::actor::bootstrap::now_once(None).await?,
                    expires_after,
                )
                .with_meta_info(crate::types::agent_store::AgentMetaInfo {
                    dht_storage_arc_half_length: 0,
                })?;
                let mut data = Vec::new();
                rmp_encode(&mut data, &agent_info)?;
                let sign_req = SignNetworkDataEvt {
                    space: space.clone(),
                    agent: agent.clone(),
                    data: Arc::new(data.clone()),
                };
                let sig = evt_sender.sign_network_data(sign_req).await?;
                let agent_info_signed = crate::types::agent_store::AgentInfoSigned::try_new(
                    (*agent).clone(),
                    sig.clone(),
                    data,
                )?;
                tracing::debug!(?agent_info, ?sig);
                evt_sender
                    .put_agent_info_signed(PutAgentInfoSignedEvt {
                        space: space.clone(),
                        agent: agent.clone(),
                        agent_info_signed: agent_info_signed.clone(),
                    })
                    .await?;
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
                            let space_b64 =
                                base64::encode_config(&space[..], base64::URL_SAFE_NO_PAD);
                            let agent_b64 =
                                base64::encode_config(&agent[..], base64::URL_SAFE_NO_PAD);
                            //println!("(MDNS) - Broadcasting of Agent {:?} ({}) in space {:?} ({} ; {})",
                            // agent, agent.get_bytes().len(), space, space.get_bytes().len(), space_b64.len());
                            // Broadcast rmp encoded agent_info_signed
                            let mut buffer = Vec::new();
                            rmp_encode(&mut buffer, &agent_info_signed)?;
                            tracing::trace!(?space_b64, ?agent_b64);
                            let handle =
                                mdns_create_broadcast_thread(space_b64, agent_b64, &buffer);
                            // store handle in self
                            mdns_handles.insert(key, handle);
                        }
                    }
                    NetworkType::QuicBootstrap => {
                        crate::spawn::actor::bootstrap::put(
                            bootstrap_service.clone(),
                            agent_info_signed,
                        )
                        .await?;
                    }
                }
            }
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
}

impl ghost_actor::GhostControlHandler for Space {}

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
        let fut = self.i_s.update_agent_info();
        let i_s = self.i_s.clone();
        let evt_sender = self.evt_sender.clone();
        match self.config.network_type {
            NetworkType::QuicMdns => {
                // Listen to MDNS service that has that space as service type
                let space_b64 = base64::encode_config(&space[..], base64::URL_SAFE_NO_PAD);
                //println!("(MDNS) - Agent {:?} ({}) joined space {:?} ({} ; {})", agent, agent.get_bytes().len(), space, space.get_bytes().len(), dna_str.len());
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
                                    let remote_agent_vec = base64::decode_config(
                                        &response.service_name[..],
                                        base64::URL_SAFE_NO_PAD,
                                    )
                                    .expect("Agent base64 decode failed");
                                    let remote_agent = Arc::new(KitsuneAgent(remote_agent_vec));
                                    //println!("(MDNS) - Peer found via MDNS: {:?})", *remote_agent);
                                    let maybe_agent_info_signed =
                                        rmp_decode(&mut &*response.buffer);
                                    if let Err(e) = maybe_agent_info_signed {
                                        tracing::error!(msg = "Failed to decode MDNS peer", ?e);
                                        continue;
                                    }
                                    let remote_agent_info_signed = maybe_agent_info_signed.unwrap();
                                    //println!("(MDNS) - Found agent_info_signed: {:?})", remote_agent_info_signed);
                                    // Add to local storage
                                    let _result = evt_sender
                                        .put_agent_info_signed(PutAgentInfoSignedEvt {
                                            space: space.clone(),
                                            agent: remote_agent,
                                            agent_info_signed: remote_agent_info_signed,
                                        })
                                        .await;
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
                let bootstrap_service = self.config.bootstrap_service.clone();
                if let Some(bootstrap_service) = bootstrap_service {
                    tokio::task::spawn(async move {
                        const START_DELAY: std::time::Duration = std::time::Duration::from_secs(1);
                        const MAX_DELAY: std::time::Duration =
                            std::time::Duration::from_secs(60 * 60);
                        let mut delay_len = START_DELAY;

                        loop {
                            tokio::time::sleep(delay_len).await;
                            if delay_len <= MAX_DELAY {
                                delay_len *= 2;
                            }

                            // TODO - this will make redundant requests to bootstrap server if multiple local agents have joined the same space.
                            if let Err(e) = super::discover::add_5_or_less_non_local_agents(
                                space.clone(),
                                agent.clone(),
                                i_s.clone(),
                                evt_sender.clone(),
                                bootstrap_service.clone(),
                            )
                            .await
                            {
                                tracing::error!(msg = "Failed to get peers from bootstrap", ?e);
                            }
                        }
                    });
                }
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
        Ok(async move { Ok(()) }.boxed().into())
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

        let discover_fut =
            discover::peer_discover(self, to_agent.clone(), from_agent.clone(), timeout_ms);

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
        mut input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        // if the user doesn't care about remote_agent_count, apply default
        match input.remote_agent_count {
            None | Some(0) => {
                input.remote_agent_count = Some(
                    self.config
                        .tuning_params
                        .default_rpc_multi_remote_agent_count as u8,
                );
            }
            _ => {}
        }

        // if the user doesn't care about timeout_ms, apply default
        match input.timeout_ms {
            None | Some(0) => {
                input.timeout_ms =
                    Some(self.config.tuning_params.default_rpc_multi_timeout_ms as u64);
            }
            _ => {}
        }

        // if the user doesn't care about race_timeout_ms, apply default
        match input.race_timeout_ms {
            None | Some(0) => {
                input.race_timeout_ms = Some(DEFAULT_RPC_MULTI_RACE_TIMEOUT_MS);
            }
            _ => {}
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
                input.remote_agent_count =
                    Some(self.config.tuning_params.default_notify_remote_agent_count as u8);
            }
            _ => {}
        }

        // if the user doesn't care about timeout_ms, apply default
        // also - if set to 0, we want to return immediately, but
        // spawn a task with that default timeout.
        let do_spawn = match input.timeout_ms {
            None | Some(0) => {
                input.timeout_ms = Some(self.config.tuning_params.default_notify_timeout_ms as u64);
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

/// A Kitsune P2p Node can track multiple "spaces" -- Non-interacting namespaced
/// areas that share common transport infrastructure for communication.
pub(crate) struct Space {
    pub(crate) space: Arc<KitsuneSpace>,
    pub(crate) this_addr: url2::Url2,
    pub(crate) i_s: ghost_actor::GhostSender<SpaceInternal>,
    pub(crate) evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    pub(crate) ep_hnd: Tx2EpHnd<wire::Wire>,
    pub(crate) local_joined_agents: HashSet<Arc<KitsuneAgent>>,
    pub(crate) config: Arc<KitsuneP2pConfig>,
    mdns_handles: HashMap<Vec<u8>, Arc<AtomicBool>>,
    mdns_listened_spaces: HashSet<String>,
}

impl Space {
    /// space constructor
    pub fn new(
        space: Arc<KitsuneSpace>,
        this_addr: url2::Url2,
        i_s: ghost_actor::GhostSender<SpaceInternal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        config: Arc<KitsuneP2pConfig>,
    ) -> Self {
        let i_s_c = i_s.clone();
        tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5 * 60)).await;
                if let Err(e) = i_s_c.update_agent_info().await {
                    tracing::error!(failed_to_update_agent_info_for_space = ?e);
                }
            }
        });

        Self {
            space,
            this_addr,
            i_s,
            evt_sender,
            ep_hnd,
            local_joined_agents: HashSet::new(),
            config,
            mdns_handles: HashMap::new(),
            mdns_listened_spaces: HashSet::new(),
        }
    }

    /// actual logic for handle_rpc_multi ...
    /// the top-level handler may or may not spawn a task for this
    #[tracing::instrument(skip(self, input))]
    fn handle_rpc_multi_inner(
        &mut self,
        input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        let actor::RpcMulti {
            space,
            from_agent,
            //basis,
            //remote_agent_count,
            //timeout_ms,
            //as_race,
            //race_timeout_ms,
            payload,
            ..
        } = input;

        // TODO - FIXME - david.b - removing the parts of this that
        // actually make remote requests. We can get this data locally
        // while we are still full sync after gossip, and the timeouts
        // are not structured correctly.
        //
        // Better to re-write as part of sharding.

        //let remote_agent_count = remote_agent_count.unwrap();
        //let timeout_ms = timeout_ms.unwrap();
        //let stage_1_timeout_ms = timeout_ms / 2;

        // as an optimization - request to all local joins
        // but don't count that toward our request total
        let local_all = self
            .local_joined_agents
            .iter()
            .map(|agent| {
                let agent = agent.clone();
                self.evt_sender
                    .call(
                        space.clone(),
                        agent.clone(),
                        from_agent.clone(),
                        payload.clone(),
                    )
                    .then(|r| async move { (r, agent) })
            })
            .collect::<Vec<_>>();

        /*
        let remote_fut = discover::message_neighborhood(
            self,
            from_agent.clone(),
            remote_agent_count,
            stage_1_timeout_ms,
            timeout_ms,
            basis,
            wire::Wire::call(
                space.clone(),
                from_agent.clone(),
                from_agent,
                payload.clone().into(),
            ),
            |a, w| match w {
                wire::Wire::CallResp(c) => Ok(actor::RpcMultiResponse {
                    agent: a,
                    response: c.data.into(),
                }),
                _ => Err(()),
            },
        )
        .instrument(tracing::debug_span!("message_neighborhood", payload = ?payload.iter().take(5).collect::<Vec<_>>()));
        */

        Ok(async move {
            let out: Vec<actor::RpcMultiResponse> = futures::future::join_all(local_all)
                .await
                .into_iter()
                .filter_map(|(r, a)| {
                    if let Ok(r) = r {
                        Some(actor::RpcMultiResponse {
                            agent: a,
                            response: r,
                        })
                    } else {
                        None
                    }
                })
                .collect();

            //out.append(&mut remote_fut.await);

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
            remote_agent_count,
            timeout_ms,
            payload,
        } = input;

        let remote_agent_count = remote_agent_count.expect("set by handle_notify_multi");
        let timeout_ms = timeout_ms.expect("set by handle_notify_multi");
        let stage_1_timeout_ms = timeout_ms / 2;

        // as an optimization - broadcast to all local joins
        // but don't count that toward our publish total
        let local_all = self
            .local_joined_agents
            .iter()
            .map(|agent| {
                self.evt_sender.notify(
                    space.clone(),
                    agent.clone(),
                    from_agent.clone(),
                    payload.clone(),
                )
            })
            .collect::<Vec<_>>();

        let remote_fut = discover::message_neighborhood(
            self,
            from_agent.clone(),
            remote_agent_count,
            stage_1_timeout_ms,
            timeout_ms,
            basis,
            wire::Wire::notify(
                space.clone(),
                from_agent.clone(),
                from_agent,
                payload.into(),
            ),
            |_, w| match w {
                wire::Wire::NotifyResp(_) => Ok(()),
                _ => Err(()),
            },
        );

        Ok(async move {
            futures::future::try_join_all(local_all).await?;

            Ok(remote_fut.await.len() as u8)
        }
        .boxed()
        .into())
    }
}
