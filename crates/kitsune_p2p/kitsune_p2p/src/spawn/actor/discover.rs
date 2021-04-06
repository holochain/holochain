#![allow(dead_code)]
use super::*;
use crate::agent_store::AgentInfo;
use ghost_actor::dependencies::must_future::MustBoxFuture;
use std::collections::HashSet;
use std::convert::TryFrom;

/// This enum represents the outcomes from peer discovery
/// - OkShortcut - the agent is locally joined, just mirror the request back out
/// - OkRemote - we were able to successfully establish a remote connection
/// - Err - we were not able to establish a connection within the timeout
pub(crate) enum PeerDiscoverResult {
    OkShortcut,
    OkRemote {
        url: url2::Url2,
        con_hnd: Tx2ConHnd<wire::Wire>,
    },
    Err(KitsuneP2pError),
}

/// attempt to establish a connection to another peer within given timeout
pub(crate) fn peer_discover(
    space: &mut Space,
    to_agent: Arc<KitsuneAgent>,
    from_agent: Arc<KitsuneAgent>,
    timeout_ms: u64,
) -> MustBoxFuture<'static, PeerDiscoverResult> {
    let i_s = space.i_s.clone();
    let evt_sender = space.evt_sender.clone();
    let ep_hnd = space.ep_hnd.clone();
    let bootstrap_service = space.config.bootstrap_service.clone();
    let space = space.space.clone();
    async move {
        // run tx.create_channel an conver success result into our return type
        let try_connect = |url: url2::Url2| async {
            let con_hnd = ep_hnd
                .get_connection(url.clone(), KitsuneTimeout::from_millis(1000 * 30))
                .await?;
            KitsuneP2pResult::Ok(PeerDiscoverResult::OkRemote { url, con_hnd })
        };

        // check if this agent is locally joined
        let check_local = || async {
            if i_s.is_agent_local(to_agent.clone()).await? {
                return Ok(PeerDiscoverResult::OkShortcut);
            }

            KitsuneP2pResult::Err("failed to connect".into())
        };

        // check if we have a reference to this agent in our peer store
        // if so, see if that url is valid via try_connect
        let check_peer_store = || async {
            if let Some(info) = evt_sender
                .get_agent_info_signed(GetAgentInfoSignedEvt {
                    space: space.clone(),
                    agent: to_agent.clone(),
                })
                .await?
            {
                let info = types::agent_store::AgentInfo::try_from(&info)?;
                let url = info
                    .as_urls_ref()
                    .get(0)
                    .ok_or_else(|| KitsuneP2pError::from("no url"))?
                    .clone();
                return try_connect(url).await;
            }

            KitsuneP2pResult::Err("failed to connect".into())
        };

        let check_network = || async {
            let nodes = get_5_or_less_non_local_agents_near_basis(
                space.clone(),
                from_agent.clone(),
                Arc::new(KitsuneBasis(to_agent.to_vec())),
                i_s.clone(),
                evt_sender.clone(),
                bootstrap_service.clone(),
            )
            .await?;

            // make an AgentInfoQuery request to the returned agents
            // return the first one to sucessfully return a result
            let (req_info, _) = futures::future::select_ok(nodes.into_iter().take(3).map(|info| {
                // grr we need to move info in but not everything else...
                // thus, we have to shadow all these with references
                let ep_hnd = &ep_hnd;
                let space = &space;
                let to_agent = &to_agent;
                async move {
                    let url = info
                        .as_urls_ref()
                        .get(0)
                        .ok_or_else(|| KitsuneP2pError::from("no url"))?
                        .clone();
                    let con_hnd = ep_hnd
                        .get_connection(url, KitsuneTimeout::from_millis(1000 * 30))
                        .await?;

                    // write the query request
                    let msg = wire::Wire::agent_info_query(
                        space.clone(),
                        Arc::new(info.as_agent_ref().clone()),
                        Some(to_agent.clone()),
                        None,
                    );
                    //KitsuneMetrics::count(KitsuneMetrics::AgentInfoQuery, msg.len());
                    let res = con_hnd
                        .request(&msg, KitsuneTimeout::from_millis(1000 * 30))
                        .await?;

                    match res {
                        wire::Wire::AgentInfoQueryResp(wire::AgentInfoQueryResp {
                            mut agent_infos,
                        }) => {
                            if agent_infos.is_empty() {
                                Err("failed to connect".into())
                            } else {
                                // if we have a result, return it
                                Ok(agent_infos.remove(0))
                            }
                        }
                        _ => KitsuneP2pResult::Err("failed to connect".into()),
                    }
                }
                .boxed()
            }))
            .await?;

            // we got a result - let's add it to our store for the future
            let _ = evt_sender
                .put_agent_info_signed(PutAgentInfoSignedEvt {
                    space: space.clone(),
                    agent: from_agent.clone(),
                    agent_info_signed: req_info.clone(),
                })
                .await;

            // we got a result, try to connect to it
            let info = types::agent_store::AgentInfo::try_from(&req_info)?;
            let url = info
                .as_urls_ref()
                .get(0)
                .ok_or_else(|| KitsuneP2pError::from("no url"))?
                .clone();
            try_connect(url).await
        };

        let start_time = std::time::Instant::now();
        let mut interval_ms = 50;

        loop {
            if let Ok(res) = check_local().await {
                return res;
            }

            if let Ok(res) = check_peer_store().await {
                return res;
            }

            if let Ok(res) = check_network().await {
                return res;
            }

            let elapsed_ms = start_time.elapsed().as_millis() as u64;
            if elapsed_ms >= timeout_ms {
                return PeerDiscoverResult::Err("timeout".into());
            }

            interval_ms *= 2;
            if interval_ms > timeout_ms - elapsed_ms {
                interval_ms = timeout_ms - elapsed_ms;
            }

            tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
        }
    }
    .boxed()
    .into()
}

/// attempt to send messages to remote nodes in a staged timeout format
#[allow(clippy::too_many_arguments)]
pub(crate) fn message_neighborhood<T, F>(
    space: &mut Space,
    from_agent: Arc<KitsuneAgent>,
    target_node_count: u8,
    stage_1_timeout_if_any_ms: u64,
    stage_2_timeout_even_if_none_ms: u64,
    // ignored while full-sync
    _basis: Arc<KitsuneBasis>,
    payload: wire::Wire,
    accept_result_cb: F,
) -> MustBoxFuture<'static, Vec<T>>
where
    T: 'static + Send,
    F: Fn(Arc<KitsuneAgent>, wire::Wire) -> Result<T, ()> + 'static + Send + Sync,
{
    let i_s = space.i_s.clone();
    let evt_sender = space.evt_sender.clone();
    let ep_hnd = space.ep_hnd.clone();
    let bootstrap_service = space.config.bootstrap_service.clone();
    let space = space.space.clone();
    let accept_result_cb = Arc::new(accept_result_cb);
    async move {
        let out = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let mut sent_to = HashSet::new();
        let start_time = std::time::Instant::now();
        let mut interval_ms = 50;

        loop {
            // It is somewhat convoluted to manage both awaits on
            // our message responses and a loop deciding to send
            // more outgoing requests simultaneously.
            //
            // as a comprimize attempting to favor readable code
            // we'll check the fetch count / timing after every full
            // iteration before deciding to send more requests.

            let fetched_count = out.lock().await.len();
            if fetched_count >= target_node_count as usize {
                break;
            }

            let elapsed_ms = start_time.elapsed().as_millis() as u64;

            if elapsed_ms >= stage_1_timeout_if_any_ms && fetched_count > 0 {
                break;
            }

            if elapsed_ms >= stage_2_timeout_even_if_none_ms {
                break;
            }

            if let Ok(nodes) = get_5_or_less_non_local_agents_near_basis(
                space.clone(),
                from_agent.clone(),
                _basis.clone(),
                i_s.clone(),
                evt_sender.clone(),
                bootstrap_service.clone(),
            )
            .await
            {
                for node in nodes {
                    let to_agent = Arc::new(node.as_agent_ref().clone());
                    if !sent_to.contains(&to_agent) {
                        sent_to.insert(to_agent.clone());
                        let url = match node.as_urls_ref().get(0) {
                            None => continue,
                            Some(url) => url.clone(),
                        };
                        let fut =
                            ep_hnd.get_connection(url, KitsuneTimeout::from_millis(1000 * 30));
                        let payload = payload.clone();
                        let accept_result_cb = accept_result_cb.clone();
                        let out = out.clone();
                        tokio::task::spawn(async move {
                            let con_hnd = fut.await?;
                            /*
                            let metric_type = match &mut payload {
                                wire::Wire::Notify(n) => {
                                    n.to_agent = to_agent.clone();
                                    KitsuneMetrics::Notify
                                }
                                wire::Wire::Call(c) => {
                                    c.to_agent = to_agent.clone();
                                    KitsuneMetrics::Call
                                }
                                _ => panic!("cannot message {:?}", payload),
                            };
                            KitsuneMetrics::count(metric_type, payload.len());
                            */
                            let res = con_hnd
                                .request(&payload, KitsuneTimeout::from_millis(1000 * 30))
                                .await?;
                            if let Ok(res) = accept_result_cb(to_agent, res) {
                                out.lock().await.push(res);
                            }

                            KitsuneP2pResult::Ok(())
                        });
                    }
                }
            }

            let elapsed_ms = start_time.elapsed().as_millis() as u64;

            if elapsed_ms >= stage_2_timeout_even_if_none_ms {
                break;
            }

            interval_ms *= 2;
            if interval_ms > stage_2_timeout_even_if_none_ms - elapsed_ms {
                interval_ms = stage_2_timeout_even_if_none_ms - elapsed_ms;
            }

            tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
        }

        let mut lock = out.lock().await;
        lock.drain(..).collect()
    }
    .boxed()
    .into()
}

/// search for agents to contact
pub(crate) fn get_5_or_less_non_local_agents_near_basis(
    space: Arc<KitsuneSpace>,
    from_agent: Arc<KitsuneAgent>,
    // ignored while full-sync
    _basis: Arc<KitsuneBasis>,
    i_s: ghost_actor::GhostSender<SpaceInternal>,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    bootstrap_service: Option<url2::Url2>,
) -> MustBoxFuture<'static, KitsuneP2pResult<HashSet<AgentInfo>>> {
    async move {
        let mut out = HashSet::new();

        if let Ok(mut list) = evt_sender
            .query_agent_info_signed(QueryAgentInfoSignedEvt {
                space: space.clone(),
                agent: from_agent.clone(),
            })
            .await
        {
            // randomize the results
            rand::seq::SliceRandom::shuffle(&mut list[..], &mut rand::thread_rng());
            for item in list {
                if let Ok(info) = AgentInfo::try_from(&item) {
                    if let Ok(is_local) = i_s
                        .is_agent_local(Arc::new(info.as_agent_ref().clone()))
                        .await
                    {
                        if !is_local {
                            out.insert(info);
                        }
                    }
                }
                if out.len() >= 5 {
                    return Ok(out);
                }
            }
        }

        if let Ok(list) = super::bootstrap::random(
            bootstrap_service,
            super::bootstrap::RandomQuery {
                space: space.clone(),
                // grap a couple extra incase they happen to be local
                limit: 8.into(),
            },
        )
        .await
        {
            for item in list {
                // TODO - someday some validation here
                if let Ok(info) = AgentInfo::try_from(&item) {
                    if let Ok(is_local) = i_s
                        .is_agent_local(Arc::new(info.as_agent_ref().clone()))
                        .await
                    {
                        if !is_local {
                            // we got a result - let's add it to our store for the future
                            let _ = evt_sender
                                .put_agent_info_signed(PutAgentInfoSignedEvt {
                                    space: space.clone(),
                                    agent: from_agent.clone(),
                                    agent_info_signed: item.clone(),
                                })
                                .await;
                            out.insert(info);
                        }
                    }
                }
                if out.len() >= 5 {
                    return Ok(out);
                }
            }
        }

        if out.is_empty() {
            return Err("could not find any peers".into());
        }

        Ok(out)
    }
    .boxed()
    .into()
}

pub(crate) fn add_5_or_less_non_local_agents(
    space: Arc<KitsuneSpace>,
    from_agent: Arc<KitsuneAgent>,
    i_s: ghost_actor::GhostSender<SpaceInternal>,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    bootstrap_service: url2::Url2,
) -> MustBoxFuture<'static, KitsuneP2pResult<()>> {
    async move {
        if let Ok(list) = super::bootstrap::random(
            Some(bootstrap_service),
            super::bootstrap::RandomQuery {
                space: space.clone(),
                limit: 8.into(),
            },
        )
        .await
        {
            for item in list {
                // TODO - someday some validation here
                if let Ok(info) = AgentInfo::try_from(&item) {
                    if let Ok(is_local) = i_s
                        .is_agent_local(Arc::new(info.as_agent_ref().clone()))
                        .await
                    {
                        if !is_local {
                            // we got a result - let's add it to our store for the future
                            let _ = evt_sender
                                .put_agent_info_signed(PutAgentInfoSignedEvt {
                                    space: space.clone(),
                                    agent: from_agent.clone(),
                                    agent_info_signed: item.clone(),
                                })
                                .await;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    .boxed()
    .into()
}
