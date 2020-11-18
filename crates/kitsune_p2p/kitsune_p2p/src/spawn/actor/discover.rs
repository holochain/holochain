#![allow(dead_code)]
use super::*;
use ghost_actor::dependencies::must_future::MustBoxFuture;
use kitsune_p2p_types::codec::Codec;
use std::convert::TryFrom;

/// This enum represents the outcomes from peer discovery
/// - OkShortcut - the agent is locally joined, just mirror the request back out
/// - OkRemote - we were able to successfully establish a remote connection
/// - Err - we were not able to establish a connection within the timeout
pub(crate) enum PeerDiscoverResult {
    OkShortcut,
    OkRemote {
        url: url2::Url2,
        write: TransportChannelWrite,
        read: TransportChannelRead,
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
    let tx = space.transport.clone();
    let space = space.space.clone();
    async move {
        // run tx.create_channel an conver success result into our return type
        let try_connect = |url| async {
            let (url, write, read) = tx.create_channel(url).await?;
            KitsuneP2pResult::Ok(PeerDiscoverResult::OkRemote { url, write, read })
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
            // this is naive while full-synced
            // just pulling 3 random nodes to query
            // eventually we'll need to pick nodes close to our target

            // first pull back our full peer store
            let mut nodes = evt_sender
                .query_agent_info_signed(QueryAgentInfoSignedEvt {
                    space: space.clone(),
                    agent: from_agent.clone(),
                })
                .await?;

            // randomize the results
            rand::seq::SliceRandom::shuffle(&mut nodes[..], &mut rand::thread_rng());

            // make an AgentInfoQuery request to 3 random agents
            // return the first one to sucessfully return a result
            let (req_info, _) = futures::future::select_ok(nodes.into_iter().take(3).map(|info| {
                // grr we need to move info in but not everything else...
                // thus, we have to shadow all these with references
                let tx = &tx;
                let space = &space;
                let to_agent = &to_agent;
                async move {
                    let info = types::agent_store::AgentInfo::try_from(&info)?;
                    let url = info
                        .as_urls_ref()
                        .get(0)
                        .ok_or_else(|| KitsuneP2pError::from("no url"))?
                        .clone();
                    let (_, mut write, read) = tx.create_channel(url).await?;

                    // write the query request
                    write
                        .write_and_close(
                            wire::Wire::agent_info_query(
                                space.clone(),
                                Arc::new(info.as_agent_ref().clone()),
                                Some(to_agent.clone()),
                                None,
                            )
                            .encode_vec()?,
                        )
                        .await?;

                    // parse the response
                    let res = read.read_to_end().await;
                    let (_, res) = wire::Wire::decode_ref(&res)?;
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
        let mut interval_ms = 10;

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

            tokio::time::delay_for(std::time::Duration::from_millis(interval_ms)).await;
        }
    }
    .boxed()
    .into()
}
