use crate::actor::BroadcastData;
use crate::dht_arc::DhtArc;
use crate::event::PutAgentInfoSignedEvt;
use crate::spawn::actor::space::WireConHnd;
use crate::spawn::actor::space::{SpaceInternal, SpaceInternalHandler, SpaceInternalHandlerResult};
use crate::spawn::actor::MaybeDelegate;
use crate::spawn::actor::OpHashList;
use crate::spawn::actor::Payload;
use crate::spawn::actor::VecMXM;
use crate::spawn::meta_net::MetaNetCon;
use crate::wire::Wire;
use crate::{GossipModuleType, KitsuneP2pError};
use futures::FutureExt;
use ghost_actor::{GhostControlHandler, GhostHandler};
use kitsune_p2p_fetch::FetchContext;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::{KAgent, KBasis, KOpHash, KSpace};
use std::collections::HashSet;

pub struct SpaceInternalStub {
    pub(crate) called_count: usize,
    pub(crate) errored_count: usize,
    pub(crate) respond_with_error: bool,
}

impl SpaceInternalStub {
    pub fn new() -> Self {
        SpaceInternalStub {
            called_count: 0,
            errored_count: 0,
            respond_with_error: false,
        }
    }
}

impl GhostControlHandler for SpaceInternalStub {}
impl GhostHandler<SpaceInternal> for SpaceInternalStub {}
impl SpaceInternalHandler for SpaceInternalStub {
    fn handle_list_online_agents_for_basis_hash(
        &mut self,
        _space: KSpace,
        _from_agent: KAgent,
        _basis: KBasis,
    ) -> SpaceInternalHandlerResult<HashSet<KAgent>> {
        unreachable!()
    }

    fn handle_update_agent_info(&mut self) -> SpaceInternalHandlerResult<()> {
        if self.respond_with_error {
            self.errored_count += 1;

            Ok(async move { Err(KitsuneP2pError::other("test error")) }
                .boxed()
                .into())
        } else {
            self.called_count += 1;

            Ok(async move { Ok(()) }.boxed().into())
        }
    }

    fn handle_update_single_agent_info(
        &mut self,
        _agent: KAgent,
    ) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_publish_agent_info_signed(
        &mut self,
        _input: PutAgentInfoSignedEvt,
    ) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_get_all_local_joined_agent_infos(
        &mut self,
    ) -> SpaceInternalHandlerResult<Vec<AgentInfoSigned>> {
        unreachable!()
    }

    fn handle_is_agent_local(&mut self, _agent: KAgent) -> SpaceInternalHandlerResult<bool> {
        unreachable!()
    }

    fn handle_update_agent_arc(
        &mut self,
        _agent: KAgent,
        _arc: DhtArc,
    ) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_incoming_delegate_broadcast(
        &mut self,
        _space: KSpace,
        _basis: KBasis,
        _to_agent: KAgent,
        _mod_idx: u32,
        _mod_cnt: u32,
        _data: BroadcastData,
    ) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_incoming_publish(
        &mut self,
        _space: KSpace,
        _to_agent: KAgent,
        _source: KAgent,
        _op_hash_list: OpHashList,
        _context: FetchContext,
        _maybe_delegate: MaybeDelegate,
    ) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_notify(&mut self, _to_agent: KAgent, _data: Wire) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_resolve_publish_pending_delegates(
        &mut self,
        _space: KSpace,
        _op_hash: KOpHash,
    ) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_incoming_gossip(
        &mut self,
        _space: KSpace,
        _con: MetaNetCon,
        _remote_url: String,
        _data: Payload,
        _module_type: GossipModuleType,
    ) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_incoming_metric_exchange(
        &mut self,
        _space: KSpace,
        _msgs: VecMXM,
    ) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_new_con(&mut self, _url: String, _con: WireConHnd) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }

    fn handle_del_con(&mut self, _url: String) -> SpaceInternalHandlerResult<()> {
        unreachable!()
    }
}
