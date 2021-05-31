#![allow(dead_code)]
use super::*;
use crate::agent_store::{AgentInfo, AgentInfoSigned};
use ghost_actor::dependencies::must_future::MustBoxFuture;
use std::collections::HashSet;
use std::convert::TryFrom;
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

/// attempt to establish a connection to another peer within given timeout
pub(crate) fn peer_discover(
    space: &mut Space,
    to_agent: Arc<KitsuneAgent>,
    _from_agent: Arc<KitsuneAgent>,
    // TODO - FIXME - upgrade to KitsuneTimeout
    timeout_ms: u64,
) -> MustBoxFuture<'static, PeerDiscoverResult> {
    let timeout = KitsuneTimeout::from_millis(timeout_ms);

    let i_s = space.i_s.clone();
    let evt_sender = space.evt_sender.clone();
    let ep_hnd = space.ep_hnd.clone();
    let space = space.space.clone();
    async move {
        // run tx.create_channel an conver success result into our return type
        let try_connect = |url: url2::Url2| async {
            let con_hnd = ep_hnd.get_connection(url.clone(), timeout).await?;
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

        let start_time = std::time::Instant::now();
        let mut interval_ms = 50;

        loop {
            if let Ok(res) = check_local().await {
                return res;
            }

            if let Ok(res) = check_peer_store().await {
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

/// local search for remote (non-local) agents closest to basis
pub(crate) fn get_cached_remotes_near_basis(
    space: &mut Space,
    basis_loc: u32,
    _timeout: KitsuneTimeout,
) -> impl Future<Output = KitsuneP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
    let i_s = space.i_s.clone();
    let evt_sender = space.evt_sender.clone();
    let space = space.space.clone();

    async move {
        let mut nodes = Vec::new();

        for node in evt_sender
            .query_agent_info_signed_near_basis(space.clone(), basis_loc, 20)
            .await?
        {
            if !i_s
                .is_agent_local(Arc::new(node.as_agent_ref().clone()))
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
                    let to_agent = Arc::new(node.as_agent_ref().clone());
                    if !sent_to.contains(&to_agent) {
                        sent_to.insert(to_agent.clone());
                        let url = match node.as_urls_ref().get(0) {
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

        if out.is_empty() {
            return Err("could not find any peers".into());
        }

        Ok(out)
    }
    .boxed()
    .into()
}
