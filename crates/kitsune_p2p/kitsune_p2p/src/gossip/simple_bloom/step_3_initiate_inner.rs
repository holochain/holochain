use super::*;

// !WARNING! - this should be sync and as fast as possible
//             the gossip mutex is locked for the duration of this fn!
pub(crate) fn danger_mutex_locked_sync_step_3_initiate_inner(
    inner: &mut SimpleBloomModInner2,
) -> KitsuneResult<()> {
    // first, check to see if we've run an initiate check too recently
    if (inner.last_initiate_check.elapsed().as_millis() as u32)
        < inner.tuning_params.gossip_loop_iteration_delay_ms
    {
        return Ok(());
    }

    // second check to see if we should time out any current initiate_tgt
    if let Some(initiate_tgt) = inner.initiate_tgt.clone() {
        if let Some(metric) = inner.remote_metrics.get(&initiate_tgt) {
            if metric.was_err
                || metric.last_touch.elapsed().as_millis() as u32
                    > inner.tuning_params.tx2_implicit_timeout_ms
            {
                tracing::warn!("gossip timeout on initiate tgt {:?}", inner.initiate_tgt);
                inner.initiate_tgt = None;
            } else {
                // we're still processing the current initiate...
                // continue.
                return Ok(());
            }
        } else {
            // erm... we have an initate tgt, but we've never seen them??
            // this must be a logic error.
            unreachable!()
        }
    }

    // get the agent-type keys
    let mut keys = inner
        .local_key_set
        .iter()
        .filter(|k| matches!(&***k, MetaOpKey::Agent(_)))
        .collect::<Vec<_>>();

    // randomize the keys
    use rand::prelude::*;
    let mut rng = thread_rng();
    keys.shuffle(&mut rng);

    // pick the first one that matches our criteria
    // or just proceed without a gossip initiate.
    let mut initiate_url = None;
    for key in keys {
        let (url, cert) = match (|| {
            if let Some(data) = inner.local_data_map.get(&**key) {
                if let MetaOpData::Agent(agent_info_signed) = &**data {
                    use std::convert::TryFrom;
                    if let Ok(agent_info) =
                        crate::agent_store::AgentInfo::try_from(agent_info_signed)
                    {
                        if let Some(url) = agent_info.as_urls_ref().get(0) {
                            if let Ok(url) = kitsune_p2p_proxy::ProxyUrl::from_full(url.as_str()) {
                                return Some((
                                    TxUrl::from(url.as_str()),
                                    Tx2Cert::from(url.digest()),
                                ));
                            }
                        }
                    }
                }
            }
            None
        })() {
            Some((url, cert)) => (url, cert),
            None => continue,
        };

        match inner.remote_metrics.entry(cert.clone()) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                // we've seen this node before, let's see if it's been too long
                let e = e.get_mut();

                let saw_recently = if e.was_err {
                    e.last_touch.elapsed().as_millis() as u32
                        <= inner
                            .tuning_params
                            .gossip_peer_on_error_next_gossip_delay_ms
                } else {
                    e.last_touch.elapsed().as_millis() as u32
                        <= inner
                            .tuning_params
                            .gossip_peer_on_success_next_gossip_delay_ms
                };

                if saw_recently {
                    // we've seen this node too recently, skip them
                    continue;
                }

                // it's been a while since we spoke to this node,
                // talk to them
                e.last_touch = std::time::Instant::now();
                e.was_err = false;
                initiate_url = Some(url);
                break;
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                // yay, we haven't seen this node, let's talk to them
                e.insert(NodeInfo {
                    last_touch: std::time::Instant::now(),
                    was_err: false,
                });
                inner.initiate_tgt = Some(cert);
                initiate_url = Some(url);
                break;
            }
        }
    }

    if let Some(url) = initiate_url {
        tracing::info!(?url, "initiating gossip");
        let gossip = encode_bloom_filter(&inner.local_bloom);
        let gossip = GossipWire::initiate(gossip);
        inner.outgoing.push((url, gossip));
    }

    Ok(())
}
