use super::*;
use kitsune_p2p_types::codec::*;

pub(crate) async fn step_4_com_loop_inner_outgoing(
    tuning_params: KitsuneP2pTuningParams,
    space: Arc<KitsuneSpace>,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    url: TxUrl,
    gossip: GossipWire,
) -> KitsuneResult<()> {
    let gossip = gossip.encode_vec().map_err(KitsuneError::other)?;
    let gossip = wire::Wire::gossip(space, gossip.into());

    let t = tuning_params.implicit_timeout();

    let con = ep_hnd.get_connection(url, t).await?;
    con.notify(&gossip, t).await?;

    Ok(())
}

pub(crate) async fn step_4_com_loop_inner_incoming(
    _inner: &Share<SimpleBloomModInner2>,
    _con: Tx2ConHnd<wire::Wire>,
    gossip: GossipWire,
) -> KitsuneResult<()> {
    match gossip {
        GossipWire::Initiate(Initiate { filter: _ }) => {}
        GossipWire::Accept(Accept { filter: _ }) => {}
        GossipWire::Chunk(Chunk { .. }) => {}
    }

    Ok(())
}
