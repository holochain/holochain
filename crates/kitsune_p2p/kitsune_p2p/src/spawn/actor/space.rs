use super::*;
use crate::types::gossip::GossipModule;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_mdns::*;
use kitsune_p2p_types::codec::{rmp_decode, rmp_encode};
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use url2::Url2;

ghost_actor::ghost_chan! {
    pub(crate) chan SpaceInternal<crate::KitsuneP2pError> {
        /// List online agents that claim to be covering a basis hash
        fn list_online_agents_for_basis_hash(space: Arc<KitsuneSpace>, from_agent: Arc<KitsuneAgent>, basis: Arc<KitsuneBasis>) -> HashSet<Arc<KitsuneAgent>>;

        /// Update / publish our agent info
        fn update_agent_info() -> ();

        /// Update / publish a single agent info
        fn update_single_agent_info(agent: Arc<KitsuneAgent>) -> ();

        /// see if an agent is locally joined
        fn is_agent_local(agent: Arc<KitsuneAgent>) -> bool;

        /// Incoming Delegate Broadcast
        fn incoming_delegate_broadcast(
            space: Arc<KitsuneSpace>,
            basis: Arc<KitsuneBasis>,
            to_agent: Arc<KitsuneAgent>,
            mod_idx: u32,
            mod_cnt: u32,
            data: crate::wire::WireData,
        ) -> ();

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
        let agent_list: Vec<Arc<KitsuneAgent>> = self.local_joined_agents.iter().cloned().collect();
        let bound_url = self.this_addr.clone();
        let evt_sender = self.evt_sender.clone();
        let bootstrap_service = self.config.bootstrap_service.clone();
        let expires_after = self.config.tuning_params.agent_info_expires_after_ms as u64;
        Ok(async move {
            let urls = vec![bound_url.into()];
            for agent in agent_list {
                let input = UpdateAgentInfoInput {
                    expires_after,
                    space: space.clone(),
                    agent,
                    urls: &urls,
                    evt_sender: &evt_sender,
                    network_type: network_type.clone(),
                    mdns_handles: &mut mdns_handles,
                    bootstrap_service: &bootstrap_service,
                };
                update_single_agent_info(input).await?;
            }
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
        let bootstrap_service = self.config.bootstrap_service.clone();
        let expires_after = self.config.tuning_params.agent_info_expires_after_ms as u64;
        Ok(async move {
            let urls = vec![bound_url.into()];
            let input = UpdateAgentInfoInput {
                expires_after,
                space: space.clone(),
                agent,
                urls: &urls,
                evt_sender: &evt_sender,
                network_type: network_type.clone(),
                mdns_handles: &mut mdns_handles,
                bootstrap_service: &bootstrap_service,
            };
            update_single_agent_info(input).await?;
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

    fn handle_incoming_delegate_broadcast(
        &mut self,
        space: Arc<KitsuneSpace>,
        basis: Arc<KitsuneBasis>,
        _to_agent: Arc<KitsuneAgent>,
        mod_idx: u32,
        mod_cnt: u32,
        data: crate::wire::WireData,
    ) -> InternalHandlerResult<()> {
        let mut local_events = Vec::new();
        for agent in self.local_joined_agents.iter().cloned() {
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

        let ro_inner = self.ro_inner.clone();
        let timeout = ro_inner.config.tuning_params.implicit_timeout();
        let fut =
            discover::get_cached_remotes_near_basis(ro_inner.clone(), basis.get_loc(), timeout);

        Ok(async move {
            futures::future::join_all(local_events).await;

            let info_list = fut.await?;

            let mut all = Vec::new();
            for info in info_list
                .into_iter()
                .filter(|info| info.agent.get_loc() % mod_cnt == mod_idx)
            {
                let ro_inner = ro_inner.clone();
                let space = space.clone();
                let basis = basis.clone();
                let data = data.clone();
                all.push(async move {
                    use discover::PeerDiscoverResult;
                    let con_hnd = match discover::peer_connect(ro_inner, &info, timeout).await {
                        PeerDiscoverResult::OkShortcut => return,
                        PeerDiscoverResult::OkRemote { con_hnd, .. } => con_hnd,
                        PeerDiscoverResult::Err(err) => {
                            tracing::warn!(?err, "broadcast error");
                            return;
                        }
                    };
                    let payload = wire::Wire::broadcast(space, basis, info.agent.clone(), data);
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
        data: Box<[u8]>,
    ) -> InternalHandlerResult<()> {
        self.gossip_mod.incoming_gossip(con, data)?;
        Ok(async move { Ok(()) }.boxed().into())
    }
}

struct UpdateAgentInfoInput<'borrow> {
    expires_after: u64,
    space: Arc<KitsuneSpace>,
    agent: Arc<KitsuneAgent>,
    urls: &'borrow Vec<TxUrl>,
    evt_sender: &'borrow futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    network_type: NetworkType,
    mdns_handles: &'borrow mut HashMap<Vec<u8>, Arc<AtomicBool>>,
    bootstrap_service: &'borrow Option<Url2>,
}

async fn update_single_agent_info(input: UpdateAgentInfoInput<'_>) -> KitsuneP2pResult<()> {
    let UpdateAgentInfoInput {
        expires_after,
        space,
        agent,
        urls,
        evt_sender,
        network_type,
        mdns_handles,
        bootstrap_service,
    } = input;
    use kitsune_p2p_types::agent_info::AgentInfoSigned;
    let signed_at_ms = crate::spawn::actor::bootstrap::now_once(None).await?;
    let expires_at_ms = signed_at_ms + expires_after;

    let agent_info_signed = AgentInfoSigned::sign(
        space.clone(),
        agent.clone(),
        u32::MAX,
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
            crate::spawn::actor::bootstrap::put(bootstrap_service.clone(), agent_info_signed)
                .await?;
        }
    }
    Ok(())
}

use ghost_actor::dependencies::must_future::MustBoxFuture;
impl ghost_actor::GhostControlHandler for Space {
    fn handle_ghost_actor_shutdown(mut self) -> MustBoxFuture<'static, ()> {
        async move {
            use futures::sink::SinkExt;
            // this is a curtesy, ok if fails
            let _ = self.evt_sender.close().await;
            self.gossip_mod.close();
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
        self.gossip_mod.local_agent_join(agent.clone());
        let fut = self.i_s.update_single_agent_info(agent);
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
        self.gossip_mod.local_agent_leave(agent.clone());
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

    #[allow(clippy::redundant_clone)] // david.b - keep adding / removing tasks
                                      //           let me keep the code
                                      //           the same without adding/
                                      //           removing clone on the last
                                      //           one every time i change it
    fn handle_rpc_multi(
        &mut self,
        input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        let RpcMulti {
            space,
            from_agent,
            basis,
            payload,
            max_remote_agent_count,
            max_timeout,
            remote_request_grace_ms,
        } = input;

        use kitsune_p2p_types::tx2::tx2_utils::*;

        // we've got a bunch of parallel tasks going on
        // this struct coordinates between them
        struct Inner {
            kill: Arc<tokio::sync::Notify>,
            got_data: Arc<tokio::sync::Notify>,
            resp: Vec<actor::RpcMultiResponse>,
            grace_rs: kitsune_p2p_types::reverse_semaphore::ReverseSemaphore,
            remain_remote_count: u8,
            already_tried: HashSet<Arc<KitsuneAgent>>,
        }

        let grace_rs = kitsune_p2p_types::reverse_semaphore::ReverseSemaphore::new();

        // capture startup permits first thing to ensure we don't
        // close things down before they even get started : )
        let local_startup_permit = grace_rs.acquire();
        let remote_startup_permit = grace_rs.acquire();

        // store this inner data in a share
        let inner = Share::new(Inner {
            kill: Arc::new(tokio::sync::Notify::new()),
            got_data: Arc::new(tokio::sync::Notify::new()),
            resp: Vec::new(),
            grace_rs,
            remain_remote_count: max_remote_agent_count,
            already_tried: HashSet::new(),
        });

        // prepare to close tasks on notification
        use std::future::Future;
        fn wrap<F>(i: Share<Inner>, f: F) -> impl Future<Output = ()> + 'static + Send
        where
            F: Future<Output = ()> + 'static + Send,
        {
            let f = f.boxed();
            let n = i
                .share_mut(|i, _| Ok(i.kill.clone()))
                .expect("we never close this share");
            async move {
                let _ = futures::future::select(f, n.notified().boxed()).await;
            }
        }

        let (driver, agg) = kitsune_p2p_types::task_agg::TaskAgg::new();

        // die on max timeout
        {
            let inner = inner.clone();
            agg.push(
                wrap(inner.clone(), async move {
                    tokio::time::sleep(max_timeout.time_remaining()).await;
                    let _ = inner.share_mut(|i, _| {
                        i.kill.notify_waiters();
                        Ok(())
                    });
                })
                .boxed(),
            );
        }

        // die when we've got the correct timing / data
        {
            let inner = inner.clone();
            agg.push(
                wrap(inner.clone(), async move {
                    let got_data = inner
                        .share_mut(|i, _| Ok(i.got_data.clone()))
                        .expect("we never close this share");

                    // first we wait for any data in the response
                    got_data.notified().await;

                    // obtain a future that resolves when we're done waiting
                    // for graceful permits
                    let grace_fut = inner
                        .share_mut(|i, _| Ok(i.grace_rs.wait_on_zero_permits()))
                        .expect("we never close this share");

                    // wait on the graceful permit future
                    grace_fut.await;

                    // we have data, and grace timeout elapsed, we can die
                    let _ = inner.share_mut(|i, _| {
                        i.kill.notify_waiters();
                        Ok(())
                    });
                })
                .boxed(),
            );
        }

        // fetch data from all local agents
        {
            let space = space.clone();
            let from_agent = from_agent.clone();
            let payload = payload.clone();
            let local_joined_agents = self.local_joined_agents.clone();
            let evt_sender = self.evt_sender.clone();
            let inner = inner.clone();
            let agg2 = agg.clone();
            agg.push(
                wrap(inner.clone(), async move {
                    // store our prev permit, so we can acquire a new one
                    // before releasing the previous one
                    let mut prev_permit = Share::new(local_startup_permit);

                    for agent in local_joined_agents {
                        let agent2 = agent.clone();
                        let permit = inner
                            .share_mut(move |i, _| {
                                i.already_tried.insert(agent2);
                                Ok(i.grace_rs.acquire())
                            })
                            .expect("we never close this share");
                        let permit = Share::new(permit);
                        let permit2 = permit.clone();

                        // drop this permit after our grace period times out
                        agg2.push(
                            async move {
                                tokio::time::sleep(std::time::Duration::from_millis(
                                    remote_request_grace_ms,
                                ))
                                .await;
                                permit2.close();
                            }
                            .boxed(),
                        );

                        // make sure our prev_permit is dropped
                        // after we pick up a permit for this iteration
                        prev_permit.close();
                        prev_permit = permit;

                        let response = match evt_sender
                            .call(
                                space.clone(),
                                agent.clone(),
                                from_agent.clone(),
                                payload.clone(),
                            )
                            .await
                        {
                            Ok(res) => res,
                            Err(err) => {
                                tracing::warn!(?err, "local rpc multi error");
                                continue;
                            }
                        };
                        inner
                            .share_mut(move |i, _| {
                                i.resp.push(RpcMultiResponse { agent, response });
                                i.got_data.notify_waiters();
                                Ok(())
                            })
                            .expect("we never close this share");
                    }
                    prev_permit.close();
                })
                .boxed(),
            );
        }

        // fetch data from remote agents
        {
            use kitsune_p2p_types::agent_info::AgentInfoSigned;

            let space = space.clone();
            let from_agent = from_agent.clone();
            let payload = payload.clone();
            //let local_joined_agents = self.local_joined_agents.clone();
            //let evt_sender = self.evt_sender.clone();
            let agg2 = agg.clone();
            let inner = inner.clone();
            let ro_inner = self.ro_inner.clone();
            agg.push(
                wrap(inner.clone(), async move {
                    let startup_permit = Share::new(remote_startup_permit);

                    let space = &space;
                    let from_agent = &from_agent;
                    let payload = &payload;
                    let agg2 = &agg2;
                    let inner = &inner;
                    let ro_inner = &ro_inner;

                    let make_req = move |info: AgentInfoSigned| {
                        async move {
                            let cont = inner.share_mut(|i, _| {
                                if i.remain_remote_count > 0 {
                                    i.remain_remote_count -= 1;
                                    Ok(true)
                                } else {
                                    Ok(false)
                                }
                            })
                            .expect("we never close this share");

                            if !cont {
                                return false;
                            }

                            let agent2 = info.agent.clone();
                            let permit = inner
                                .share_mut(move |i, _| {
                                    i.already_tried.insert(agent2);
                                    Ok(i.grace_rs.acquire())
                                })
                                .expect("we never close this share");
                            let permit = Share::new(permit);
                            let permit2 = permit.clone();

                            // drop this permit after our grace period times out
                            agg2.push(
                                async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(
                                        remote_request_grace_ms,
                                    ))
                                    .await;
                                    permit2.close();
                                }
                                .boxed(),
                            );

                            let space = space.clone();
                            let from_agent = from_agent.clone();
                            let payload = payload.clone();
                            let inner = inner.clone();
                            let ro_inner = ro_inner.clone();
                            agg2.push(async move {
                                use discover::PeerDiscoverResult;
                                // now we need to defer this to task aggregation
                                let con_hnd = match discover::peer_connect(
                                    ro_inner.clone(),
                                    &info,
                                    max_timeout,
                                ).await {
                                    PeerDiscoverResult::OkShortcut => {
                                        permit.close();
                                        return;
                                    }
                                    PeerDiscoverResult::Err(_) => {
                                        permit.close();
                                        return;
                                    }
                                    PeerDiscoverResult::OkRemote { con_hnd, .. } => con_hnd,
                                };

                                let msg = wire::Wire::call(
                                    space.clone(),
                                    from_agent.clone(),
                                    info.agent.clone(),
                                    payload.clone().into(),
                                );

                                let res = con_hnd.request(&msg, max_timeout).await;
                                match res {
                                    Ok(wire::Wire::CallResp(c)) => {
                                        let agent = info.agent.clone();
                                        let response = c.data.into();
                                        inner
                                            .share_mut(move |i, _| {
                                                i.resp.push(RpcMultiResponse { agent, response });
                                                i.got_data.notify_waiters();
                                                Ok(())
                                            })
                                            .expect("we never close this share");
                                    }
                                    _ => (),
                                }

                                permit.close();
                            }.boxed());

                            true
                        }
                    };

                    if let Ok(infos) = discover::get_cached_remotes_near_basis(
                        ro_inner.clone(),
                        basis.get_loc(),
                        max_timeout,
                    ).await {
                        for info in infos {
                            make_req(info).await;
                        }
                    }

                    // we've initiated our requests...
                    // we can let our startup permit lapse
                    startup_permit.close();
                })
                .boxed(),
            );
        }

        Ok(async move {
            driver.await;

            let Inner { resp, .. } = inner
                .try_unwrap()
                // this should never happen...
                // all other copies die with the join_all/kill above
                .unwrap_or(None)
                // this should never happen...
                // we never close the share anywhere
                .expect("failed to unwrap shared");

            Ok(resp)
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
        let mut local_events = Vec::new();
        for agent in self.local_joined_agents.iter().cloned() {
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
            tokio::task::spawn(async move {
                let cover_nodes = discover_fut.await?;
                if cover_nodes.is_empty() {
                    return Err("failed to discover neighboring peers".into());
                }

                let mut all = Vec::new();

                // is there a better way to do this??
                let half_timeout =
                    KitsuneTimeout::from_millis(timeout.time_remaining().as_millis() as u64 / 2);
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

                let mod_cnt = con_list.len();
                for (mod_idx, (agent, con_hnd)) in con_list.into_iter().enumerate() {
                    let payload = wire::Wire::delegate_broadcast(
                        space.clone(),
                        basis.clone(),
                        agent,
                        mod_idx as u32,
                        mod_cnt as u32,
                        payload.clone().into(),
                    );
                    all.push(async move {
                        if let Err(err) = con_hnd.notify(&payload, timeout).await {
                            tracing::warn!(?err, "delegate broadcast error");
                        }
                    });
                }

                futures::future::join_all(all).await;

                KitsuneP2pResult::Ok(())
            });

            Ok(())
        }
        .boxed()
        .into())
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
                            for item in list {
                                // TODO - someday some validation here
                                let agent = item.agent.clone();
                                match i_s_c.is_agent_local(agent.clone()).await {
                                    Err(err) => tracing::error!(?err),
                                    Ok(is_local) => {
                                        if !is_local {
                                            // we got a result - let's add it to our store for the future
                                            if let Err(err) = evt_s_c
                                                .put_agent_info_signed(PutAgentInfoSignedEvt {
                                                    space: space_c.clone(),
                                                    agent,
                                                    agent_info_signed: item.clone(),
                                                })
                                                .await
                                            {
                                                tracing::error!(
                                                    ?err,
                                                    "error storing bootstrap agent_info"
                                                );
                                            }
                                        }
                                    }
                                }
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
        });

        Self {
            ro_inner,
            space,
            this_addr,
            i_s,
            evt_sender,
            local_joined_agents: HashSet::new(),
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
            use kitsune_p2p_types::agent_info::AgentInfoSigned;
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
                    agent: agent.clone(),
                    agent_info_signed: agent_info_signed.clone(),
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
}
