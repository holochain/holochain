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
    inner: &Share<SimpleBloomModInner2>,
    con: Tx2ConHnd<wire::Wire>,
    gossip: GossipWire,
) -> KitsuneResult<()> {
    let (send_accept, remote_filter) = match gossip {
        GossipWire::Initiate(Initiate { filter }) => (true, filter),
        GossipWire::Accept(Accept { filter }) => (false, filter),
        GossipWire::Chunk(Chunk { .. }) => {
            return Ok(());
        }
    };

    let remote_filter = decode_bloom_filter(&remote_filter);

    let _out_keys = inner.share_mut(move |i, _| {
        // for now, just always accept gossip initiates
        if send_accept {
            let local_filter = encode_bloom_filter(&i.local_bloom);
            let gossip = GossipWire::accept(local_filter);
            i.outgoing
                .push((con.peer_cert(), HowToConnect::Con(con), gossip));
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

    Ok(())
}
