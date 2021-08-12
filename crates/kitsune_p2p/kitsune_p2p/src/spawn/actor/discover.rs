#![allow(dead_code)]
use super::*;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
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
                                if let Err(err) = inner
                                    .evt_sender
                                    .put_agent_info_signed(PutAgentInfoSignedEvt {
                                        space: inner.space.clone(),
                                        peer_data: vec![agent_info_signed.clone()],
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
            //let s_remain = timeout.time_remaining().as_secs_f64();
            //tracing::trace!(%s_remain, "search_remotes_covering_basis iteration");

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
                            if let Err(err) = inner
                                .evt_sender
                                .put_agent_info_signed(PutAgentInfoSignedEvt {
                                    space: inner.space.clone(),
                                    peer_data: peer_list,
                                })
                                .await
                            {
                                tracing::error!(?err, "error storing peer_queried agent_info");
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
