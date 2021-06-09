use super::*;

pub(crate) async fn step_3_initiate_inner(bloom: &SimpleBloomMod) -> KitsuneP2pResult<()> {
    // we have decided to do an initiate check, mark the time

    // get the remote certs we might want to speak to
    let endpoints: HashMap<GossipTgt, TxUrl> = bloom.inner.share_mut(|inner, _| {
        inner.last_initiate_check = std::time::Instant::now();
        // TODO: In the future we'll pull the endpoints from a p2p store query that
        //       finds nodes which overlap our arc.
        //       For now we use `local_data_map`.
        Ok(inner
            .local_data_map
            .values()
            .filter_map(|v| {
                if let MetaOpData::Agent(agent_info_signed) = &**v {
                    // this is for remote gossip, we've already sync local agents
                    if inner
                        .local_agents
                        .contains(agent_info_signed.as_agent_ref())
                    {
                        return None;
                    }

                    use std::convert::TryFrom;
                    if let Ok(agent_info) =
                        crate::agent_store::AgentInfo::try_from(agent_info_signed)
                    {
                        if let Some(url) = agent_info.as_urls_ref().get(0) {
                            if let Ok(purl) = kitsune_p2p_proxy::ProxyUrl::from_full(url.as_str()) {
                                return Some((
                                    GossipTgt::new(
                                        vec![Arc::new(agent_info.as_agent_ref().clone())],
                                        Tx2Cert::from(purl.digest()),
                                    ),
                                    TxUrl::from(url.as_str()),
                                ));
                            }
                        }
                    }
                }
                None
            })
            .collect())
    })?;
    let mut endpoints: Vec<(GossipTgt, TxUrl)> = endpoints.into_iter().collect();

    let last_touch_fudge_ms: u32 = {
        use rand::prelude::*;
        let mut rng = thread_rng();
        // randomize the keys
        endpoints.shuffle(&mut rng);
        // last_touch fudge
        // we don't really want two nodes to both decide to initiate gossip
        // at the same time... so let's randomize our talk window by a
        // couple seconds
        rng.gen_range(0, 5000)
    };

    // pick the first one that matches our criteria
    // or just proceed without a gossip initiate.
    let mut initiate = None;

    for (endpoint, url) in endpoints {
        match bloom.get_metric(endpoint.agents().clone()).await? {
            Some(info) => {
                // we've seen this node before, let's see if it's been too long

                let saw_recently = if info.was_err {
                    info.last_touch.elapsed()?.as_millis() as u32 + last_touch_fudge_ms
                        <= bloom
                            .tuning_params
                            .gossip_peer_on_error_next_gossip_delay_ms
                } else {
                    info.last_touch.elapsed()?.as_millis() as u32 + last_touch_fudge_ms
                        <= bloom
                            .tuning_params
                            .gossip_peer_on_success_next_gossip_delay_ms
                };

                if saw_recently {
                    tracing::trace!(?endpoint, "saw too recently");
                    // we've seen this node too recently, skip them
                    continue;
                }

                // it's been a while since we spoke to this node,
                // talk to them
                bloom.inner.share_mut(|inner, _| {
                    inner.record_pending_metric(endpoint.agents().clone(), false);
                    Ok(())
                })?;
                initiate = Some((endpoint, url));

                break;
            }
            None => {
                // yay, we haven't seen this node, let's talk to them
                bloom.inner.share_mut(|inner, _| {
                    inner.record_pending_metric(endpoint.agents().clone(), false);
                    inner.initiate_tgt = Some(endpoint.clone());
                    Ok(())
                })?;
                initiate = Some((endpoint, url));
                break;
            }
        }
    }

    bloom.inner.share_mut(|inner, _| {
        if let Some((endpoint, url)) = initiate {
            let gossip = encode_bloom_filter(&inner.local_bloom);
            let bloom_byte_count = gossip.len();
            tracing::info!(%url, ?endpoint, %bloom_byte_count, "initiating gossip");
            let gossip = GossipWire::initiate(inner.local_agents.clone(), gossip);
            inner
                .outgoing
                .push((endpoint, HowToConnect::Url(url), gossip));
        }
        Ok(())
    })?;

    Ok(())
}
