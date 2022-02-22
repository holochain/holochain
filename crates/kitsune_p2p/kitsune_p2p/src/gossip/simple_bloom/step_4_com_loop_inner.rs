use super::*;
use kitsune_p2p_types::codec::*;

impl SimpleBloomMod {
    pub(crate) async fn step_4_com_loop_inner(&self) -> KitsuneResult<()> {
        let loop_start = proc_count_now_us();

        loop {
            let (tuning_params, space, ep_hnd) = (
                self.tuning_params.clone(),
                self.space.clone(),
                self.ep_hnd.clone(),
            );
            let (mut maybe_outgoing, mut maybe_incoming) = self.inner.share_mut(|i, _| {
                let maybe_outgoing = if !i.outgoing.is_empty()
                    && proc_count_us_elapsed(i.last_outgoing_us).as_millis() as u64
                        > self.send_interval_ms
                {
                    let (cert, how, gossip) = i.outgoing.remove(0);

                    // set this to a time in the future
                    // so we don't accidentally double up if sending
                    // is slow... we'll set this more reasonably
                    // when we get a success or failure below.
                    i.last_outgoing_us = proc_count_now_us()
                        + (self.tuning_params.tx2_implicit_timeout_ms as i64 * 1000);

                    Some((cert, how, gossip))
                } else {
                    None
                };
                let maybe_incoming = if !i.incoming.is_empty() {
                    Some(i.incoming.remove(0))
                } else {
                    None
                };
                Ok((maybe_outgoing, maybe_incoming))
            })?;

            let will_break = (maybe_outgoing.is_none() && maybe_incoming.is_none())
                || proc_count_us_elapsed(loop_start).as_millis() as u32
                    > tuning_params.gossip_loop_iteration_delay_ms;

            if let Some(outgoing) = maybe_outgoing.take() {
                let (endpoint, how, gossip) = outgoing;
                let agents = endpoint.agents().clone();
                if let Err(e) = step_4_com_loop_inner_outgoing(
                    &self.inner,
                    tuning_params.clone(),
                    space.clone(),
                    ep_hnd,
                    endpoint.cert().clone(),
                    how,
                    gossip,
                )
                .await
                {
                    tracing::warn!("failed to send outgoing: {:?} {:?}", endpoint, e);
                    self.inner.share_mut(move |i, _| {
                        i.last_outgoing_us = proc_count_now_us();
                        i.record_pending_metric(agents, true);
                        Ok(())
                    })?;
                } else {
                    self.inner.share_mut(move |i, _| {
                        i.last_outgoing_us = proc_count_now_us();
                        i.record_pending_metric(agents, false);
                        Ok(())
                    })?;
                }
            }

            if let Some(incoming) = maybe_incoming.take() {
                let (con, remote_url, gossip) = incoming;
                if let Err(e) = step_4_com_loop_inner_incoming(self, con, remote_url, gossip).await
                {
                    tracing::warn!("failed to process incoming: {:?}", e);
                }
            }

            if will_break {
                break;
            }
        }
        Ok(())
    }
}

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
    let gossip = wire::Wire::gossip(space, gossip.into(), GossipModuleType::Simple);

    let t = tuning_params.implicit_timeout();

    let con = match how {
        HowToConnect::Con(con, _) => {
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
    remote_url: TxUrl,
    gossip: GossipWire,
) -> KitsuneResult<()> {
    use crate::event::*;

    // parse the message
    let (send_accept, remote_filter, remote_agents) = match gossip {
        GossipWire::Initiate(Initiate { agents, filter }) => {
            let bloom_byte_count = filter.len();
            tracing::debug!(
                %bloom_byte_count,
                "incoming 'Initiate'",
            );

            (true, filter, agents)
        }
        GossipWire::Accept(Accept { agents, filter }) => {
            let bloom_byte_count = filter.len();
            tracing::debug!(
                %bloom_byte_count,
                "incoming 'Accept'",
            );

            (false, filter, agents)
        }
        GossipWire::Chunk(Chunk {
            agents: _remote_agents,
            finished,
            chunks,
        }) => {
            let chunk_count = chunks.len();
            tracing::info!(
                %finished,
                %chunk_count,
                "incoming 'Chunk'",
            );

            // parse/integrate the chunks
            let futs = bloom.inner.share_mut(move |i, _| {
                if let Some(endpoint) = i.initiate_tgt.clone() {
                    if finished && con.peer_cert() == *endpoint.cert() {
                        i.initiate_tgt = None;
                    }
                }

                let mut futs = Vec::new();

                // Locally sync the newly received data
                let mut to_send_ops = Vec::new();
                let mut to_send_peer_data = Vec::new();
                for chunk in &chunks {
                    match &**chunk {
                        MetaOpData::Op(key, data) => {
                            to_send_ops.push((key.clone(), data.clone()));
                        }
                        MetaOpData::Agent(agent_info_signed) => {
                            // TODO - we actually only need to do this
                            // once, since the agent store is shared...
                            to_send_peer_data.push(agent_info_signed.clone());
                        }
                    }
                    let key = chunk.key();
                    i.local_bloom.set(&key);
                    i.local_data_map.insert(key, chunk.clone());
                }
                if !to_send_ops.is_empty() {
                    futs.push(bloom.evt_sender.gossip(bloom.space.clone(), to_send_ops));
                }
                if !to_send_peer_data.is_empty() {
                    futs.push(
                        bloom
                            .evt_sender
                            .put_agent_info_signed(PutAgentInfoSignedEvt {
                                space: bloom.space.clone(),
                                peer_data: to_send_peer_data,
                            }),
                    );
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

    let remote_agents: Vec<_> = remote_agents.into_iter().collect();
    let remote_filter = decode_bloom_filter(&remote_filter);

    // send accept if applicable / gather the keys the remote needs
    let con_clone = con.clone();
    let remote_agents_clone = remote_agents.clone();
    let remote_url_clone = remote_url.clone();
    let out_keys = bloom.inner.share_mut(move |i, _| {
        // for now, just always accept gossip initiates
        if send_accept {
            let local_filter = encode_bloom_filter(&i.local_bloom);
            let gossip = GossipWire::accept(i.local_agents.clone(), local_filter);
            let peer_cert = con_clone.peer_cert();
            let endpoint = GossipTgt::new(remote_agents_clone, peer_cert);
            i.outgoing.push((
                endpoint,
                HowToConnect::Con(con_clone, remote_url_clone),
                gossip,
            ));
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
        let remote_agents_clone = remote_agents.clone();
        let remote_url_clone = remote_url.clone();
        bloom.inner.share_mut(move |i, _| {
            if let Some(tgt) = i.initiate_tgt.clone() {
                if con.peer_cert() == *tgt.cert() {
                    i.initiate_tgt = None;
                }
            }

            // publish an empty chunk in case it was the remote who initiated
            let gossip = GossipWire::chunk(i.local_agents.clone(), true, Vec::new());
            let peer_cert = con.peer_cert();
            let endpoint = GossipTgt::new(remote_agents_clone, peer_cert);
            i.outgoing
                .push((endpoint, HowToConnect::Con(con, remote_url_clone), gossip));

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
            let gossip = GossipWire::chunk(i.local_agents.clone(), finished, chunks);
            let peer_cert = con.peer_cert();
            let endpoint = GossipTgt::new(remote_agents.clone(), peer_cert);
            i.outgoing.push((
                endpoint,
                HowToConnect::Con(con.clone(), remote_url.clone()),
                gossip,
            ));
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
                if let Some(url) = agent_info_signed.url_list.get(0) {
                    if let Ok(purl) = kitsune_p2p_proxy::ProxyUrl::from_full(url.as_str()) {
                        if &Tx2Cert::from(purl.digest()) != cert {
                            continue;
                        }

                        if agent_info_signed.signed_at_ms < most_recent {
                            continue;
                        }
                        most_recent = agent_info_signed.signed_at_ms;

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
    let (space, _agent, maybe_data) = bloom.inner.share_mut(|i, _| {
        // erm, just using a random agent??
        Ok((
            bloom.space.clone(),
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
        .fetch_op_data(FetchOpDataEvt {
            space,
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
