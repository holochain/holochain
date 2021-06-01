use super::*;
use crate::types::gossip::GossipModule;
use ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::tracing_futures::Instrument;
use kitsune_p2p_mdns::*;
use kitsune_p2p_types::codec::{rmp_decode, rmp_encode};
use std::collections::{HashMap, HashSet};
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

        /// Incoming Gossip
        fn incoming_gossip(space: Arc<KitsuneSpace>, con: Tx2ConHnd<wire::Wire>, data: Box<[u8]>) -> ();
    }
}

pub(crate) async fn spawn_space(
    space: Arc<KitsuneSpace>,
    this_addr: url2::Url2,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    config: Arc<KitsuneP2pConfig>,
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
    )));

    Ok((sender, i_s, evt_recv))
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

    fn handle_incoming_gossip(
        &mut self,
        _space: Arc<KitsuneSpace>,
        con: Tx2ConHnd<wire::Wire>,
        data: Box<[u8]>,
    ) -> InternalHandlerResult<()> {
        self.gossip_mod.incoming_gossip(con, data)?;
        Ok(async move { Ok(()) }.boxed().into())
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
        self.gossip_mod.local_agent_join(agent);
        let fut = self.i_s.update_agent_info();
        //let i_s = self.i_s.clone();
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
        self.gossip_mod.local_agent_leave(agent);
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
            discover::search_and_discover_peer_connect(
                self, to_agent.clone(), timeout);

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
    gossip_mod: GossipModule,
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
        let gossip_mod_fact = if &config.tuning_params.gossip_strategy == "simple-bloom" {
            crate::gossip::simple_bloom::factory()
        } else {
            panic!(
                "unknown gossip strategy: {}",
                config.tuning_params.gossip_strategy
            );
        };
        let gossip_mod = gossip_mod_fact.spawn_gossip_task(
            config.tuning_params.clone(),
            space.clone(),
            ep_hnd.clone(),
            evt_sender.clone(),
        );

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
                    tokio::time::sleep(delay_len).await;
                    if delay_len <= MAX_DELAY {
                        delay_len *= 2;
                    }

                    match super::bootstrap::random(
                        bootstrap_service.clone(),
                        super::bootstrap::RandomQuery {
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
                            for item in list {
                                // TODO - someday some validation here
                                use std::convert::TryFrom;
                                if let Ok(info) = crate::agent_store::AgentInfo::try_from(&item) {
                                    let agent = Arc::new(info.as_agent_ref().clone());
                                    if let Ok(is_local) = i_s_c.is_agent_local(agent.clone()).await
                                    {
                                        if !is_local {
                                            // we got a result - let's add it to our store for the future
                                            let _ = evt_s_c
                                                .put_agent_info_signed(PutAgentInfoSignedEvt {
                                                    space: space_c.clone(),
                                                    agent,
                                                    agent_info_signed: item.clone(),
                                                })
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }

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
            gossip_mod,
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
