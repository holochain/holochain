use crate::dht::PeerView;
use crate::dht_arc::DhtArc;
use crate::event::{
    FetchOpDataEvt, KitsuneP2pEvent, KitsuneP2pEventHandler, KitsuneP2pEventHandlerResult,
    PutAgentInfoSignedEvt, QueryAgentsEvt, QueryOpHashesEvt, SignNetworkDataEvt,
    TimeWindowInclusive,
};
use crate::{KOp, KitsuneSignature};
use ghost_actor::{GhostControlHandler, GhostHandler};
use kitsune_p2p_types::agent_info::AgentInfoSigned;

pub struct HostEventStub {}

impl HostEventStub {
    pub fn new() -> Self {
        HostEventStub {}
    }
}

impl GhostControlHandler for HostEventStub {}
impl GhostHandler<KitsuneP2pEvent> for HostEventStub {}
impl KitsuneP2pEventHandler for HostEventStub {
    fn handle_put_agent_info_signed(
        &mut self,
        input: PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        todo!()
    }

    fn handle_query_agents(
        &mut self,
        input: QueryAgentsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<AgentInfoSigned>> {
        todo!()
    }

    fn handle_query_peer_density(
        &mut self,
        space: crate::types::event::KSpace,
        dht_arc: DhtArc,
    ) -> KitsuneP2pEventHandlerResult<PeerView> {
        todo!()
    }

    fn handle_call(
        &mut self,
        space: crate::types::event::KSpace,
        to_agent: crate::types::event::KAgent,
        payload: crate::types::event::Payload,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        todo!()
    }

    fn handle_notify(
        &mut self,
        space: crate::types::event::KSpace,
        to_agent: crate::types::event::KAgent,
        payload: crate::types::event::Payload,
    ) -> KitsuneP2pEventHandlerResult<()> {
        todo!()
    }

    fn handle_receive_ops(
        &mut self,
        space: crate::types::event::KSpace,
        ops: crate::types::event::Ops,
        context: crate::types::event::MaybeContext,
    ) -> KitsuneP2pEventHandlerResult<()> {
        todo!()
    }

    fn handle_query_op_hashes(
        &mut self,
        input: QueryOpHashesEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<(Vec<kitsune_p2p_types::KOpHash>, TimeWindowInclusive)>>
    {
        todo!()
    }

    fn handle_fetch_op_data(
        &mut self,
        input: FetchOpDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(kitsune_p2p_types::KOpHash, KOp)>> {
        todo!()
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        todo!()
    }
}
