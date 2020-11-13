use crate::agent_store::AgentInfoSigned;

use super::*;
use ghost_actor::dependencies::{tracing, tracing_futures::Instrument};
use kitsune_p2p_types::codec::Codec;
use std::{collections::HashSet, convert::TryFrom};

/// if the user specifies None or zero (0) for remote_agent_count
const DEFAULT_NOTIFY_REMOTE_AGENT_COUNT: u8 = 5;

/// if the user specifies None or zero (0) for timeout_ms
const DEFAULT_NOTIFY_TIMEOUT_MS: u64 = 1000;

/// if the user specifies None or zero (0) for timeout_ms
const DEFAULT_RPC_SINGLE_TIMEOUT_MS: u64 = 2000;

/// if the user specifies None or zero (0) for remote_agent_count
const DEFAULT_RPC_MULTI_REMOTE_AGENT_COUNT: u8 = 2;

/// if the user specifies None or zero (0) for timeout_ms
const DEFAULT_RPC_MULTI_TIMEOUT_MS: u64 = 20;

/// if the user specifies None or zero (0) for race_timeout_ms
const DEFAULT_RPC_MULTI_RACE_TIMEOUT_MS: u64 = 200;

/// Agent info can expire 20 minutes after it is signed.
/// This is somewhat arbitrary and open to tweaking.
pub const AGENT_INFO_EXPIRES_AFTER_MS: u64 = 60 * 1000 * 20;

ghost_actor::ghost_chan! {
    pub(crate) chan SpaceInternal<crate::KitsuneP2pError> {
        /// Make a remote request right-now if we have an open connection,
        /// otherwise, return an error.
        fn immediate_request(space: Arc<KitsuneSpace>, to_agent: Arc<KitsuneAgent>, from_agent: Arc<KitsuneAgent>, data: Arc<Vec<u8>>) -> wire::Wire;

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
    transport: ghost_actor::GhostSender<TransportListener>,
    config: Arc<KitsuneP2pConfig>,
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

    let i_s = builder
        .channel_factory()
        .create_channel::<SpaceInternal>()
        .await?;

    let sender = builder
        .channel_factory()
        .create_channel::<KitsuneP2p>()
        .await?;

    tokio::task::spawn(builder.spawn(Space::new(space, i_s, evt_send, transport, config)));

    Ok((sender, evt_recv))
}

impl ghost_actor::GhostHandler<gossip::GossipEvent> for Space {}

impl gossip::GossipEventHandler for Space {
    fn handle_list_neighbor_agents(
        &mut self,
    ) -> gossip::GossipEventHandlerResult<Vec<Arc<KitsuneAgent>>> {
        // while full-sync this is just a clone of list_by_basis
        let all_agents = self
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
            let mut peer_store = fut
                .await?
                .into_iter()
                .map(|ai| Arc::new(ai.into_agent()))
                .filter(|a| !all_agents.contains(a))
                .collect::<Vec<_>>();
            peer_store.extend(all_agents);
            Ok(peer_store)
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
            Ok(async move { fut.await }.boxed().into())
        } else {
            let ReqOpHashesEvt {
                to_agent,
                dht_arc,
                since_utc_epoch_s,
                until_utc_epoch_s,
                from_agent,
            } = input;
            let transport_tx = self.transport.clone();
            let evt_sender = self.evt_sender.clone();
            let space = self.space.clone();
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
                )
                .encode_vec()?;
                let info = types::agent_store::AgentInfo::try_from(&info)?;
                let url = info.as_urls_ref().get(0).unwrap().clone();
                let (_, mut write, read) = transport_tx.create_channel(url).await?;
                write.write_and_close(data.to_vec()).await?;
                let read = read.read_to_end().await;
                let (_, read) = wire::Wire::decode_ref(&read)?;
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
            let transport_tx = self.transport.clone();
            let evt_sender = self.evt_sender.clone();
            let space = self.space.clone();
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
                    wire::Wire::fetch_op_data(space, from_agent, to_agent, op_hashes, peer_hashes)
                        .encode_vec()?;
                let info = types::agent_store::AgentInfo::try_from(&info)?;
                let url = info.as_urls_ref().get(0).unwrap().clone();
                let (_, mut write, read) = transport_tx.create_channel(url).await?;
                write.write_and_close(data.to_vec()).await?;
                let read = read.read_to_end().await;
                let (_, read) = wire::Wire::decode_ref(&read)?;
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

    fn handle_gossip_ops(
        &mut self,
        from_agent: Arc<KitsuneAgent>,
        to_agent: Arc<KitsuneAgent>,
        ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
        agents: Vec<AgentInfoSigned>,
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
        let all_agents = agents
            .into_iter()
            .map(|agent_info_signed| {
                self.evt_sender
                    .put_agent_info_signed(PutAgentInfoSignedEvt {
                        space: self.space.clone(),
                        agent: to_agent.clone(),
                        agent_info_signed,
                    })
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
            futures::stream::iter(all_agents)
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

pub fn local_req_op_hashes(
    evt_sender: &futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    space: Arc<KitsuneSpace>,
    input: ReqOpHashesEvt,
) -> impl std::future::Future<Output = Result<OpHashesAgentHashes, KitsuneP2pError>> {
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

impl ghost_actor::GhostHandler<SpaceInternal> for Space {}

impl SpaceInternalHandler for Space {
    fn handle_immediate_request(
        &mut self,
        _space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        data: Arc<Vec<u8>>,
    ) -> SpaceInternalHandlerResult<wire::Wire> {
        let space = self.space.clone();
        if self.local_joined_agents.contains(&to_agent) {
            // LOCAL SHORT CIRCUIT! - just forward data locally

            let evt_sender = self.evt_sender.clone();

            let (_, data) = wire::Wire::decode_ref(&data)?;

            match data {
                wire::Wire::Call(payload) => Ok(async move {
                    let res = evt_sender
                        .call(space, to_agent, from_agent, payload.data.into())
                        .await?;
                    Ok(wire::Wire::call_resp(res.into()))
                }
                .instrument(tracing::debug_span!("wire_call"))
                .boxed()
                .into()),
                wire::Wire::Notify(payload) => Ok(async move {
                    evt_sender
                        .notify(space, to_agent, from_agent, payload.data.into())
                        .await?;
                    Ok(wire::Wire::notify_resp())
                }
                .boxed()
                .into()),
                _ => unimplemented!(),
            }
        } else {
            let evt_sender = self.evt_sender.clone();
            let tx = self.transport.clone();
            Ok(async move {
                // see if we have an entry for this agent in our agent_store
                let info = match evt_sender
                    .get_agent_info_signed(GetAgentInfoSignedEvt {
                        space,
                        agent: to_agent.clone(),
                    })
                    .await?
                {
                    None => return Err(KitsuneP2pError::RoutingAgentError(to_agent)),
                    Some(i) => i,
                };
                let info = types::agent_store::AgentInfo::try_from(&info)?;
                let url = info.as_urls_ref().get(0).unwrap().clone();
                let (_, mut write, read) = tx.create_channel(url).await?;
                write.write_and_close(data.to_vec()).await?;
                let read = read.read_to_end().await;
                let (_, read) = wire::Wire::decode_ref(&read)?;
                match read {
                    wire::Wire::Failure(wire::Failure { reason }) => Err(reason.into()),
                    _ => Ok(read),
                }
            }
            .boxed()
            .into())
        }
    }

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
        let agent_list: Vec<Arc<KitsuneAgent>> = self.local_joined_agents.iter().cloned().collect();
        let bound_url = self.transport.bound_url();
        let evt_sender = self.evt_sender.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        Ok(async move {
            let bound_url = bound_url.await?;
            let urls = bound_url
                .query_pairs()
                .map(|(_, sub_url)| url2::url2!("{}", sub_url))
                .collect::<Vec<_>>();
            for agent in agent_list {
                let agent_info = crate::types::agent_store::AgentInfo::new(
                    (*space).clone(),
                    (*agent).clone(),
                    urls.clone(),
                    crate::spawn::actor::bootstrap::now_once(None).await?,
                    AGENT_INFO_EXPIRES_AFTER_MS,
                );
                let mut data = Vec::new();
                kitsune_p2p_types::codec::rmp_encode(&mut data, &agent_info)?;
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
                        agent,
                        agent_info_signed: agent_info_signed.clone(),
                    })
                    .await?;

                // Push to the bootstrap as well.
                crate::spawn::actor::bootstrap::put(bootstrap_service.clone(), agent_info_signed)
                    .await?;
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
        unreachable!("These requests are handled at the to actor level and are never propagated down to the space.")
    }

    fn handle_join(
        &mut self,
        _space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        self.local_joined_agents.insert(agent);
        let fut = self.i_s.update_agent_info();
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
            None | Some(0) => DEFAULT_RPC_SINGLE_TIMEOUT_MS,
            _ => timeout_ms.unwrap(),
        };

        let discover_fut =
            discover::peer_discover(self, to_agent.clone(), from_agent.clone(), timeout_ms);

        Ok(async move {
            match discover_fut.await {
                discover::PeerDiscoverResult::OkShortcut => {
                    // reflect this request locally
                    evt_sender.call(space, to_agent, from_agent, payload).await
                }
                discover::PeerDiscoverResult::OkRemote {
                    mut write, read, ..
                } => {
                    let payload = wire::Wire::call(
                        space.clone(),
                        from_agent.clone(),
                        to_agent.clone(),
                        payload.into(),
                    )
                    .encode_vec()?;
                    write.write_and_close(payload).await?;
                    let res = read.read_to_end().await;
                    let (_, res) = wire::Wire::decode_ref(&res)?;
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

/// A Kitsune P2p Node can track multiple "spaces" -- Non-interacting namespaced
/// areas that share common transport infrastructure for communication.
pub(crate) struct Space {
    pub(crate) space: Arc<KitsuneSpace>,
    pub(crate) i_s: ghost_actor::GhostSender<SpaceInternal>,
    pub(crate) evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    pub(crate) transport: ghost_actor::GhostSender<TransportListener>,
    pub(crate) local_joined_agents: HashSet<Arc<KitsuneAgent>>,
    pub(crate) config: Arc<KitsuneP2pConfig>,
}

impl Space {
    /// space constructor
    pub fn new(
        space: Arc<KitsuneSpace>,
        i_s: ghost_actor::GhostSender<SpaceInternal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
        transport: ghost_actor::GhostSender<TransportListener>,
        config: Arc<KitsuneP2pConfig>,
    ) -> Self {
        let i_s_c = i_s.clone();
        tokio::task::spawn(async move {
            loop {
                tokio::time::delay_for(std::time::Duration::from_secs(5 * 60)).await;
                if i_s_c.update_agent_info().await.is_err() {
                    break;
                }
            }
        });
        Self {
            space,
            i_s,
            evt_sender,
            transport,
            local_joined_agents: HashSet::new(),
            config,
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
            timeout_ms,
            //as_race,
            //race_timeout_ms,
            payload,
            ..
        } = input;
        let timeout_ms = timeout_ms.unwrap();

        // TODO - we cannot write proper logic here until we have a
        //        proper peer discovery mechanism. Instead, let's
        //        give it 100 ms max to see if there is any agent
        //        other than us - prefer that, or fall back to
        //        just reflecting the msg to ourselves.

        let i_s = self.i_s.clone();
        Ok(async move {
            let mut to_agent = from_agent.clone();
            'search_loop: for _ in 0..5 {
                if let Ok(agent_list) = i_s
                    .list_online_agents_for_basis_hash(
                        space.clone(),
                        from_agent.clone(),
                        basis.clone(),
                    )
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

            // encode the data to send
            let payload = Arc::new(
                wire::Wire::call(
                    space.clone(),
                    from_agent.clone(),
                    to_agent.clone(),
                    payload.into(),
                )
                .encode_vec()?,
            );

            // Timeout on immediate requests after a small interval.
            // TODO: 20 ms is only appropriate for local calls and not
            // real networking
            if let Ok(Ok(response)) = tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                i_s.immediate_request(space, to_agent.clone(), from_agent.clone(), payload),
            )
            .await
            {
                if let wire::Wire::CallResp(wire::CallResp { data }) = response {
                    out.push(actor::RpcMultiResponse {
                        agent: to_agent,
                        response: data.into(),
                    });
                }
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
            // always trying to notify 2 remote agents at the moment
            remote_agent_count: _,
            timeout_ms,
            payload,
        } = input;

        let timeout_ms = timeout_ms.expect("set by handle_notify_multi");

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

        let i_s = self.i_s.clone();
        let evt_sender = self.evt_sender.clone();
        let tx = self.transport.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        Ok(async move {
            futures::future::try_join_all(local_all).await?;

            let mut sent_to: HashSet<Arc<KitsuneAgent>> = HashSet::new();
            let send_success_count = Arc::new(std::sync::atomic::AtomicU8::new(0));

            let start_time = std::time::Instant::now();
            let mut interval_ms = 10;

            loop {
                if let Ok(nodes) = discover::get_5_or_less_non_local_agents_near_basis(
                    space.clone(),
                    from_agent.clone(),
                    basis.clone(),
                    i_s.clone(),
                    evt_sender.clone(),
                    bootstrap_service.clone(),
                )
                .await
                {
                    let mut all = Vec::new();

                    for node in nodes {
                        let to_agent = Arc::new(node.as_agent_ref().clone());

                        if !sent_to.contains(&to_agent) {
                            sent_to.insert(to_agent.clone());

                            let ssc = send_success_count.clone();
                            let tx = tx.clone();
                            let space = &space;
                            let from_agent = &from_agent;
                            let payload = &payload;
                            all.push(async move {
                                let url = node
                                    .as_urls_ref()
                                    .get(0)
                                    .ok_or_else(|| KitsuneP2pError::from("no url"))?
                                    .clone();
                                let (_, mut write, read) = tx.create_channel(url).await?;
                                write
                                    .write_and_close(
                                        wire::Wire::notify(
                                            space.clone(),
                                            from_agent.clone(),
                                            to_agent.clone(),
                                            payload.clone().into(),
                                        )
                                        .encode_vec()?,
                                    )
                                    .await?;
                                let res = read.read_to_end().await;
                                let (_, res) = wire::Wire::decode_ref(&res)?;
                                if let wire::Wire::NotifyResp(_) = res {
                                    ssc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                }
                                KitsuneP2pResult::Ok(())
                            });
                        }
                    }

                    if !all.is_empty() {
                        let _ = futures::future::join_all(all).await;
                    }
                }

                if send_success_count.load(std::sync::atomic::Ordering::Relaxed) >= 2 {
                    break;
                }

                let elapsed_ms = start_time.elapsed().as_millis() as u64;
                if elapsed_ms >= timeout_ms {
                    break;
                }

                interval_ms *= 2;
                if interval_ms > timeout_ms - elapsed_ms {
                    interval_ms = timeout_ms - elapsed_ms;
                }

                tokio::time::delay_for(std::time::Duration::from_millis(interval_ms)).await;
            }

            Ok(send_success_count.load(std::sync::atomic::Ordering::Relaxed))
        }
        .boxed()
        .into())
    }
}
