use super::*;

pub(crate) async fn step_4_com_loop_inner_outgoing(
    _space: Arc<KitsuneSpace>,
    _ep_hnd: Tx2EpHnd<wire::Wire>,
    _outgoing: (TxUrl, GossipWire),
) -> KitsuneResult<()> {
    Ok(())
}

pub(crate) async fn step_4_com_loop_inner_incoming(
    _space: Arc<KitsuneSpace>,
    _evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    _incoming: (Tx2ConHnd<wire::Wire>, GossipWire),
) -> KitsuneResult<()> {
    Ok(())
}
