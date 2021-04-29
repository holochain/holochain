use super::*;
use kitsune_p2p_types::codec::*;

pub(crate) async fn step_4_com_loop_inner_outgoing(
    tuning_params: KitsuneP2pTuningParams,
    space: Arc<KitsuneSpace>,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    how: HowToConnect,
    gossip: GossipWire,
) -> KitsuneResult<()> {
    let gossip = gossip.encode_vec().map_err(KitsuneError::other)?;
    let gossip = wire::Wire::gossip(space, gossip.into());

    let t = tuning_params.implicit_timeout();

    let con = match how {
        HowToConnect::Con(con) => con,
        HowToConnect::Url(url) => ep_hnd.get_connection(url, t).await?,
    };
    con.notify(&gossip, t).await?;

    Ok(())
}

pub(crate) async fn step_4_com_loop_inner_incoming(
    inner: &Share<SimpleBloomModInner>,
    con: Tx2ConHnd<wire::Wire>,
    gossip: GossipWire,
) -> KitsuneResult<()> {
    use crate::event::*;

    // parse the message
    let (send_accept, remote_filter) = match gossip {
        GossipWire::Initiate(Initiate { filter }) => (true, filter),
        GossipWire::Accept(Accept { filter }) => (false, filter),
        GossipWire::Chunk(Chunk { finished, chunks }) => {
            // parse/integrate the chunks
            let futs = inner.share_mut(move |i, _| {
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
                                futs.push(i.evt_sender.gossip(
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
                                futs.push(i.evt_sender.put_agent_info_signed(
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
    let out_keys = inner.share_mut(move |i, _| {
        // for now, just always accept gossip initiates
        if send_accept {
            let local_filter = encode_bloom_filter(&i.local_bloom);
            let gossip = GossipWire::accept(local_filter);
            i.outgoing
                .push((con_clone.peer_cert(), HowToConnect::Con(con_clone), gossip));
        }

        let mut out_keys = Vec::new();

        // find the keys for data the remote doesn't have
        for key in i.local_key_set.iter() {
            if !remote_filter.check(key) {
                out_keys.push(key.clone());
            }
        }

        Ok(out_keys)
    })?;

    // get all the local data we have that they need
    let mut out_data = Vec::new();
    for key in out_keys {
        if let Some(data) = data_map_get(inner, &key).await? {
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

    inner.share_mut(move |i, _| {
        for (finished, chunks) in gossip {
            let gossip = GossipWire::chunk(finished, chunks);
            i.outgoing
                .push((con.peer_cert(), HowToConnect::Con(con.clone()), gossip));
        }

        Ok(())
    })?;

    Ok(())
}

async fn data_map_get(
    inner: &Share<SimpleBloomModInner>,
    key: &Arc<MetaOpKey>,
) -> KitsuneResult<Option<Arc<MetaOpData>>> {
    use crate::event::*;

    // first, see if we already have the data
    let (space, agent, evt_sender, maybe_data) = inner.share_mut(|i, _| {
        // erm, just using a random agent??
        Ok((
            i.space.clone(),
            i.local_agents.iter().next().unwrap().clone(),
            i.evt_sender.clone(),
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
        MetaOpKey::Agent(_) => unreachable!(),
    };

    // next, check locally
    let mut op = match evt_sender
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
    inner.share_mut(|i, _| {
        i.local_data_map.insert(fetched_key, data.clone());
        Ok(())
    })?;

    Ok(Some(data))
}
