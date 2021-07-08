#![allow(dead_code)]
use super::*;
//use ghost_actor::dependencies::must_future::MustBoxFuture;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
//use std::collections::HashSet;
use std::future::Future;

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

/// search for / discover / and open a connection to a specific remote agent
pub(crate) fn search_and_discover_peer_connect(
    inner: Arc<SpaceReadOnlyInner>,
    to_agent: Arc<KitsuneAgent>,
    timeout: KitsuneTimeout,
) -> impl Future<Output = PeerDiscoverResult> + 'static + Send {
    const INITIAL_DELAY: u64 = 100;
    const MAX_DELAY: u64 = 1000;

    async move {
        let backoff = timeout.backoff(INITIAL_DELAY, MAX_DELAY);
        loop {
            // see if the tgt agent is actually local
            if let Ok(true) = inner.i_s.is_agent_local(to_agent.clone()).await {
                return PeerDiscoverResult::OkShortcut;
            }

            // see if we already know how to reach the tgt agent
            if let Ok(Some(agent_info_signed)) = inner
                .evt_sender
                .get_agent_info_signed(GetAgentInfoSignedEvt {
                    space: inner.space.clone(),
                    agent: to_agent.clone(),
                })
                .await
            {
                return peer_connect(inner.clone(), &agent_info_signed, timeout).await;
            }

            // the next step involves making network requests
            // so check our timeout first
            if timeout.is_expired() {
                return PeerDiscoverResult::Err("timeout discovering peer".into());
            }

            // let's do some discovery
            if let Ok(nodes) =
                search_remotes_covering_basis(inner.clone(), to_agent.get_loc(), timeout).await
            {
                for node in nodes {
                    // try connecting to the returned nodes
                    if let PeerDiscoverResult::OkRemote { con_hnd, .. } =
                        peer_connect(inner.clone(), &node, timeout).await
                    {
                        // make a peer query for the basis
                        let payload = wire::Wire::peer_get(inner.space.clone(), to_agent.clone());
                        match con_hnd.request(&payload, timeout).await {
                            Ok(wire::Wire::PeerGetResp(wire::PeerGetResp {
                                agent_info_signed,
                            })) => {
                                let agent = agent_info_signed.agent.clone();
                                if let Err(err) = inner
                                    .evt_sender
                                    .put_agent_info_signed(PutAgentInfoSignedEvt {
                                        space: inner.space.clone(),
                                        agent,
                                        agent_info_signed: agent_info_signed.clone(),
                                    })
                                    .await
                                {
                                    tracing::error!(
                                        ?err,
                                        "search_and_discover error putting agent info"
                                    );
                                }

                                // hey, we got our target node info
                                // return the try-to-connect future
                                return peer_connect(inner, &agent_info_signed, timeout).await;
                            }
                            peer_resp => {
                                tracing::warn!(?peer_resp, "unexpected peer resp");
                            }
                        }
                    }
                }
            }

            backoff.wait().await;
        }
    }
}

/// attempt to establish a connection to another peer within given timeout
pub(crate) fn peer_connect(
    inner: Arc<SpaceReadOnlyInner>,
    agent_info_signed: &AgentInfoSigned,
    timeout: KitsuneTimeout,
) -> impl Future<Output = PeerDiscoverResult> + 'static + Send {
    let agent = agent_info_signed.agent.clone();
    let url = agent_info_signed
        .url_list
        .get(0)
        .cloned()
        .ok_or_else(|| KitsuneP2pError::from("no url - agent is likely offline"));

    async move {
        let url = url?;

        // if they are local, return the shortcut result
        if inner.i_s.is_agent_local(agent).await? {
            return Ok(PeerDiscoverResult::OkShortcut);
        }

        // attempt an outgoing connection
        let con_hnd = inner.ep_hnd.get_connection(url.clone(), timeout).await?;

        // return the result
        Ok(PeerDiscoverResult::OkRemote {
            url: url.into(),
            con_hnd,
        })
    }
    .map(|r| match r {
        Ok(r) => r,
        Err(e) => PeerDiscoverResult::Err(e),
    })
}

/// looping search for agents covering basis_loc
/// by requesting closer agents from remote nodes
pub(crate) fn search_remotes_covering_basis(
    inner: Arc<SpaceReadOnlyInner>,
    basis_loc: u32,
    timeout: KitsuneTimeout,
) -> impl Future<Output = KitsuneP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
    const INITIAL_DELAY: u64 = 100;
    const MAX_DELAY: u64 = 1000;
    const CHECK_NODE_COUNT: usize = 8;

    async move {
        let backoff = timeout.backoff(INITIAL_DELAY, MAX_DELAY);
        loop {
            let mut cover_nodes = Vec::new();
            let mut near_nodes = Vec::new();

            // first check our local peer store,
            // sort into nodes covering the basis and otherwise
            for node in get_cached_remotes_near_basis(inner.clone(), basis_loc, timeout)
                .await
                .unwrap_or_else(|_| Vec::new())
            {
                if node.storage_arc.contains(basis_loc) {
                    cover_nodes.push(node);
                } else {
                    near_nodes.push(node);
                }
                if cover_nodes.len() + near_nodes.len() >= CHECK_NODE_COUNT {
                    break;
                }
            }

            // if we have any nodes covering the basis, return them
            if !cover_nodes.is_empty() {
                return Ok(cover_nodes);
            }

            // if we've exhausted our timeout, we should exit
            timeout.ok()?;

            if near_nodes.is_empty() {
                // maybe just wait and try again?
                backoff.wait().await;
                continue;
            }

            // shuffle the returned nodes so we don't keep hammering the same one
            use rand::prelude::*;
            near_nodes.shuffle(&mut rand::thread_rng());

            let mut added_data = false;
            for node in near_nodes {
                // try connecting to the returned nodes
                if let PeerDiscoverResult::OkRemote { con_hnd, .. } =
                    peer_connect(inner.clone(), &node, timeout).await
                {
                    // make a peer query for the basis
                    let payload = wire::Wire::peer_query(inner.space.clone(), basis_loc);
                    match con_hnd.request(&payload, timeout).await {
                        Ok(wire::Wire::PeerQueryResp(wire::PeerQueryResp { peer_list })) => {
                            if peer_list.is_empty() {
                                tracing::warn!("empty discovery peer list");
                                continue;
                            }
                            // if we got results, add them to our peer store
                            for agent_info_signed in peer_list {
                                let agent = agent_info_signed.agent.clone();
                                if let Err(err) = inner
                                    .evt_sender
                                    .put_agent_info_signed(PutAgentInfoSignedEvt {
                                        space: inner.space.clone(),
                                        agent,
                                        agent_info_signed,
                                    })
                                    .await
                                {
                                    tracing::error!(?err, "error storing peer_queried agent_info");
                                }
                            }
                            // then break, to pull up the local query
                            // that should now include these new results
                            added_data = true;
                            break;
                        }
                        peer_resp => {
                            tracing::warn!(?peer_resp, "unexpected peer resp");
                        }
                    }
                }
            }

            if !added_data {
                backoff.wait().await;
            }
        }
    }
}

/// local search for remote (non-local) agents closest to basis
pub(crate) fn get_cached_remotes_near_basis(
    inner: Arc<SpaceReadOnlyInner>,
    basis_loc: u32,
    _timeout: KitsuneTimeout,
) -> impl Future<Output = KitsuneP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
    // as this is a local request, there isn't much cost to getting more
    // results than we strictly need
    const LIMIT: u32 = 20;

    async move {
        let mut nodes = Vec::new();

        for node in inner
            .evt_sender
            .query_agent_info_signed_near_basis(inner.space.clone(), basis_loc, LIMIT)
            .await?
        {
            if !inner
                .i_s
                .is_agent_local(node.agent.clone())
                .await
                .unwrap_or(true)
            {
                nodes.push(node);
            }
        }

        if nodes.is_empty() {
            return Err("no remote nodes found, abort discovery".into());
        }

        Ok(nodes)
    }
}

/*
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
    // TODO - FIXME - this maybe we want to init two timeouts here
    //                for the if any / even if none distinction?
    let timeout_even_if_none = KitsuneTimeout::from_millis(stage_2_timeout_even_if_none_ms);

    let i_s = space.i_s.clone();
    let evt_sender = space.evt_sender.clone();
    let ep_hnd = space.ep_hnd.clone();
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
            )
            .await
            {
                for node in nodes {
                    let to_agent = node.agent.clone();
                    if !sent_to.contains(&to_agent) {
                        sent_to.insert(to_agent.clone());
                        let url = match node.url_list.get(0) {
                            None => continue,
                            Some(url) => url.clone(),
                        };
                        let fut = ep_hnd.get_connection(url, timeout_even_if_none);
                        let mut payload = payload.clone();
                        let accept_result_cb = accept_result_cb.clone();
                        let out = out.clone();
                        tokio::task::spawn(async move {
                            let con_hnd = fut.await?;
                            match &mut payload {
                                wire::Wire::Notify(n) => {
                                    n.to_agent = to_agent.clone();
                                }
                                wire::Wire::Call(c) => {
                                    c.to_agent = to_agent.clone();
                                }
                                _ => panic!("cannot message {:?}", payload),
                            }
                            let res = con_hnd.request(&payload, timeout_even_if_none).await?;
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
) -> MustBoxFuture<'static, KitsuneP2pResult<HashSet<AgentInfoSigned>>> {
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
                match i_s.is_agent_local(item.agent.clone()).await {
                    Ok(is_local) => {
                        if !is_local {
                            out.insert(item);
                        }
                    }
                    Err(err) => tracing::error!(?err),
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
*/
