use crate::actor::BroadcastData;
use crate::spawn::actor::{EvtRcv, InternalHandlerResult, MaybeDelegate, OpHashList, VecMXM};
use crate::spawn::meta_net::MetaNetCon;
use crate::spawn::{Internal, InternalHandler};
use crate::GossipModuleType;
use ghost_actor::{GhostControlHandler, GhostHandler};
use kitsune_p2p_fetch::{FetchContext, FetchKey, FetchSource};
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::KOpHash;

pub struct InternalStub {}

impl InternalStub {
    pub fn new() -> Self {
        InternalStub {}
    }
}

impl GhostControlHandler for InternalStub {}
impl GhostHandler<Internal> for InternalStub {}
impl InternalHandler for InternalStub {
    fn handle_register_space_event_handler(&mut self, _recv: EvtRcv) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_incoming_delegate_broadcast(
        &mut self,
        _space: crate::spawn::actor::KSpace,
        _basis: crate::spawn::actor::KBasis,
        _to_agent: crate::spawn::actor::KAgent,
        _mod_idx: u32,
        _mod_cnt: u32,
        _data: BroadcastData,
    ) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_incoming_publish(
        &mut self,
        _space: crate::spawn::actor::KSpace,
        _to_agent: crate::spawn::actor::KAgent,
        _source: crate::spawn::actor::KAgent,
        _op_hash_list: OpHashList,
        _context: FetchContext,
        _maybe_delegate: MaybeDelegate,
    ) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_resolve_publish_pending_delegates(
        &mut self,
        _space: crate::spawn::actor::KSpace,
        _op_hash: KOpHash,
    ) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_incoming_gossip(
        &mut self,
        _space: crate::spawn::actor::KSpace,
        _con: MetaNetCon,
        _remote_url: String,
        _data: crate::spawn::actor::Payload,
        _module_type: GossipModuleType,
    ) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_incoming_metric_exchange(
        &mut self,
        _space: crate::spawn::actor::KSpace,
        _msgs: VecMXM,
    ) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_new_con(&mut self, _url: String, _con: MetaNetCon) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_del_con(&mut self, _url: String) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_fetch(
        &mut self,
        _key: FetchKey,
        _space: crate::spawn::actor::KSpace,
        _source: FetchSource,
    ) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_get_all_local_joined_agent_infos(
        &mut self,
    ) -> InternalHandlerResult<Vec<AgentInfoSigned>> {
        todo!()
    }
}
