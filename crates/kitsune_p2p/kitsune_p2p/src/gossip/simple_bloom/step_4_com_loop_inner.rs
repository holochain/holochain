use super::*;
use kitsune_p2p_types::codec::*;

pub(crate) async fn step_4_com_loop_inner_outgoing(
    inner: &Share<SimpleBloomModInner>,
    tuning_params: KitsuneP2pTuningParams,
    space: Arc<KitsuneSpace>,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    peer_cert: Tx2Cert,
    how: HowToConnect,
    gossip: GossipWire,
) -> KitsuneResult<()> {
    let gossip = gossip.encode_vec().map_err(KitsuneError::other)?;
    let gossip = wire::Wire::gossip(space, gossip.into());

    let t = tuning_params.implicit_timeout();

    let con = match how {
        HowToConnect::Con(con) => {
            if con.is_closed() {
                let url = pick_url_for_cert(inner, &peer_cert)?;
                ep_hnd.get_connection(url, t).await?
            } else {
                con
            }
        }
        HowToConnect::Url(url) => ep_hnd.get_connection(url, t).await?,
    };
    con.notify(&gossip, t).await?;

    Ok(())
}

pub(crate) async fn step_4_com_loop_inner_incoming(
    bloom: &SimpleBloomMod,
    con: Tx2ConHnd<wire::Wire>,
    gossip: GossipWire,
) -> KitsuneResult<()> {
    use crate::event::*;

    // parse the message
    let (send_accept, remote_filter) = match gossip {
        GossipWire::Initiate(Initiate { filter }) => {
            let bloom_byte_count = filter.len();
            tracing::debug!(
                %bloom_byte_count,
                "incoming 'Initiate'",
            );

            (true, filter)
        }
        GossipWire::Accept(Accept { filter }) => {
            let bloom_byte_count = filter.len();
            tracing::debug!(
                %bloom_byte_count,
                "incoming 'Accept'",
            );

            (false, filter)
        }
        GossipWire::Chunk(Chunk { finished, chunks }) => {
            let chunk_count = chunks.len();
            tracing::info!(
                %finished,
                %chunk_count,
                "incoming 'Chunk'",
            );

            // parse/integrate the chunks
            let futs = bloom.inner.share_mut(move |i, _| {
                if let Some(tgt_cert) = i.initiate_tgt.clone() {
                    if finished && con.peer_cert() == tgt_cert {
                        i.initiate_tgt = None;
                    }
                }

                let mut futs = Vec::new();

                for chunk in chunks {
                    for agent in i.local_agents.iter() {
                        match &*chunk {
                            MetaOpData::Op(key, data) => {
                                futs.push(bloom.evt_sender.gossip(
                                    i.space.clone(),
                                    agent.clone(),
                                    agent.clone(), // TODO - from??
                                    key.clone(),
                                    data.clone(),
                                ));
                            }
                            MetaOpData::Agent(agent_info_signed) => {
                                // TODO - we actually only need to do this
                                // once, since the agent store is shared...
                                futs.push(bloom.evt_sender.put_agent_info_signed(
                                    PutAgentInfoSignedEvt {
                                        space: i.space.clone(),
                                        agent: agent.clone(),
                                        agent_info_signed: agent_info_signed.clone(),
                                    },
                                ));
                            }
                        }
                    }
                    let key = chunk.key();
                    i.local_bloom.set(&key);
                    i.local_data_map.insert(key, chunk);
                }

                Ok(futs)
            })?;

            if !futs.is_empty() {
                futures::future::try_join_all(futs)
                    .await
                    .map_err(KitsuneError::other)?;
            }

            return Ok(());
        }
    };

    let remote_filter = decode_bloom_filter(&remote_filter);

    // send accept if applicable / gather the keys the remote needs
    let con_clone = con.clone();
    let out_keys = bloom.inner.share_mut(move |i, _| {
        // for now, just always accept gossip initiates
        if send_accept {
            let local_filter = encode_bloom_filter(&i.local_bloom);
            let gossip = GossipWire::accept(local_filter);
            let peer_cert = con_clone.peer_cert();
            i.outgoing
                .push((peer_cert, HowToConnect::Con(con_clone), gossip));
        }

        let mut out_keys = Vec::new();

        // find the keys for data the remote doesn't have
        for key in i.local_key_set.iter() {
            if !remote_filter.check(key) {
                out_keys.push(key.clone());
            }
        }

        let local_op_count = i.local_key_set.len();
        let remote_needs_op_count = out_keys.len();
        tracing::debug!(%local_op_count, %remote_needs_op_count, "identified ops remote needs");

        Ok(out_keys)
    })?;

    if out_keys.is_empty() {
        // the remote doesn't need anything from us
        // ... if we initiated this gossip, mark it as done.
        bloom.inner.share_mut(move |i, _| {
            if let Some(tgt_cert) = i.initiate_tgt.clone() {
                if con.peer_cert() == tgt_cert {
                    i.initiate_tgt = None;
                }
            }

            // publish an empty chunk incase it was the remote who initiated
            let gossip = GossipWire::chunk(true, Vec::new());
            let peer_cert = con.peer_cert();
            i.outgoing.push((peer_cert, HowToConnect::Con(con), gossip));

            Ok(())
        })?;

        return Ok(());
    }

    // get all the local data we have that they need
    let mut out_data = Vec::new();
    for key in out_keys {
        if let Some(data) = data_map_get(bloom, &key).await? {
            out_data.push(data);
        }
    }

    if out_data.is_empty() {
        return Ok(());
    }

    // build up / queue up outgoing messages with the data
    let mut gossip = Vec::new();
    let mut chunks = Vec::new();
    let mut cur_size = 0;

    macro_rules! bundle {
        () => {
            #[allow(unused_assignments)]
            {
                if !chunks.is_empty() {
                    gossip.push((false, chunks.drain(..).collect::<Vec<_>>()));
                }

                cur_size = 0;
            }
        };
    }

    for data in out_data {
        let size = data.byte_count();

        if !chunks.is_empty() && cur_size + size > MAX_SEND_BUF_BYTES {
            bundle!();
        }

        chunks.push(data);
    }
    bundle!();

    let last_idx = gossip.len() - 1;
    gossip[last_idx].0 = true;

    bloom.inner.share_mut(move |i, _| {
        for (finished, chunks) in gossip {
            let gossip = GossipWire::chunk(finished, chunks);
            let peer_cert = con.peer_cert();
            i.outgoing
                .push((peer_cert, HowToConnect::Con(con.clone()), gossip));
        }

        Ok(())
    })?;

    Ok(())
}

// if the connection is closed while awaiting an outgoing send,
// we'll need to establish a new one...
fn pick_url_for_cert(inner: &Share<SimpleBloomModInner>, cert: &Tx2Cert) -> KitsuneResult<TxUrl> {
    // this is a bit computationally intensive...
    // but, in case there is a split in data recency, safer as a starting point

    // first, gather the most recent agent_infos we have for this cert,
    // then, see if the urls match - outputting a warning if they don't
    // and pick one..

    inner.share_mut(|i, _| {
        let mut most_recent = 0;
        let mut out_url = None;
        for data in i.local_data_map.values() {
            if let MetaOpData::Agent(agent_info_signed) = &**data {
                use std::convert::TryFrom;
                if let Ok(agent_info) = crate::agent_store::AgentInfo::try_from(agent_info_signed) {
                    if let Some(url) = agent_info.as_urls_ref().get(0) {
                        if let Ok(purl) = kitsune_p2p_proxy::ProxyUrl::from_full(url.as_str()) {
                            if &Tx2Cert::from(purl.digest()) != cert {
                                continue;
                            }

                            if agent_info.signed_at_ms() < most_recent {
                                continue;
                            }
                            most_recent = agent_info.signed_at_ms();

                            let url = TxUrl::from(url.as_str());

                            if let Some(out_url) = out_url {
                                if out_url != url {
                                    tracing::warn!(?cert, %out_url, %url, "url mismatch for tgt cert");
                                }
                            }

                            out_url = Some(url);
                        }
                    }
                }
            }
        }
        match out_url.take() {
            Some(out_url) => Ok(out_url),
            None => Err("failed to find url for cert".into()),
        }
    })
}

async fn data_map_get(
    bloom: &SimpleBloomMod,
    key: &Arc<MetaOpKey>,
) -> KitsuneResult<Option<Arc<MetaOpData>>> {
    use crate::event::*;

    // first, see if we already have the data
    let (space, agent, maybe_data) = bloom.inner.share_mut(|i, _| {
        // erm, just using a random agent??
        Ok((
            i.space.clone(),
            i.local_agents.iter().next().unwrap().clone(),
            i.local_data_map.get(key).cloned(),
        ))
    })?;

    // if so, return it
    if maybe_data.is_some() {
        return Ok(maybe_data);
    }

    let op_key = match &**key {
        MetaOpKey::Op(key) => key.clone(),
        // we should already have all this data...
        MetaOpKey::Agent(_, _) => unreachable!(),
    };

    // next, check locally
    let mut op = match bloom
        .evt_sender
        .fetch_op_hash_data(FetchOpHashDataEvt {
            space,
            agent,
            op_hashes: vec![op_key],
        })
        .await
    {
        Err(_) => return Ok(None),
        Ok(op) => op,
    };

    if op.is_empty() {
        return Ok(None);
    }

    let (fetched_key, data) = op.remove(0);
    let data = Arc::new(MetaOpData::Op(fetched_key.clone(), data));
    let fetched_key = Arc::new(MetaOpKey::Op(fetched_key));
    assert_eq!(key, &fetched_key);

    // store it before returning it
    bloom.inner.share_mut(|i, _| {
        i.local_data_map.insert(fetched_key, data.clone());
        Ok(())
    })?;

    Ok(Some(data))
}
