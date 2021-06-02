use super::*;

// !WARNING! - this should be sync and as fast as possible
//             the gossip mutex is locked for the duration of this fn!
pub(crate) async fn step_3_initiate_inner(bloom: &SimpleBloomMod) -> KitsuneResult<()> {
    // we have decided to do an initiate check, mark the time

    // get the remote certs we might want to speak to
    let certs: HashMap<Tx2Cert, TxUrl> = bloom.inner.share_mut(|inner, _| {
        inner.last_initiate_check = std::time::Instant::now();
        // TODO: In the future we'll pull the certs from a p2p store query that
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
                                    Tx2Cert::from(purl.digest()),
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
    let mut certs: Vec<(Tx2Cert, TxUrl)> = certs.into_iter().collect();

    // randomize the keys
    use rand::prelude::*;
    let mut rng = thread_rng();
    certs.shuffle(&mut rng);

    // last_touch fudge
    // we don't really want two nodes to both decide to initiate gossip
    // at the same time... so let's randomize our talk window by a
    // couple seconds
    let last_touch_fudge_ms: u32 = rng.gen_range(0, 5000);

    // pick the first one that matches our criteria
    // or just proceed without a gossip initiate.
    let mut initiate = None;

    bloom.inner.share_mut(|inner, _| {
        for (cert, url) in certs {
            match inner.remote_metrics.entry(cert.clone()) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    // we've seen this node before, let's see if it's been too long
                    let e = e.get_mut();

                    let saw_recently = if e.was_err {
                        e.last_touch.elapsed().as_millis() as u32 + last_touch_fudge_ms
                            <= bloom
                                .tuning_params
                                .gossip_peer_on_error_next_gossip_delay_ms
                    } else {
                        e.last_touch.elapsed().as_millis() as u32 + last_touch_fudge_ms
                            <= bloom
                                .tuning_params
                                .gossip_peer_on_success_next_gossip_delay_ms
                    };

                    if saw_recently {
                        tracing::trace!(?cert, "saw too recently");
                        // we've seen this node too recently, skip them
                        continue;
                    }

                    // it's been a while since we spoke to this node,
                    // talk to them
                    e.last_touch = std::time::Instant::now();
                    e.was_err = false;
                    initiate = Some((cert, url));
                    break;
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    // yay, we haven't seen this node, let's talk to them
                    e.insert(NodeInfo {
                        last_touch: std::time::Instant::now(),
                        was_err: false,
                    });
                    inner.initiate_tgt = Some(cert.clone());
                    initiate = Some((cert, url));
                    break;
                }
            }
        }

        if let Some((cert, url)) = initiate {
            let gossip = encode_bloom_filter(&inner.local_bloom);
            let bloom_byte_count = gossip.len();
            tracing::info!(%url, ?cert, %bloom_byte_count, "initiating gossip");
            let gossip = GossipWire::initiate(gossip);
            inner.outgoing.push((cert, HowToConnect::Url(url), gossip));
        }
        Ok(())
    })?;

    Ok(())
}
