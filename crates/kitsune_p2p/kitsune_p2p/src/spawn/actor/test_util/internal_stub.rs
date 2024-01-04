use crate::actor::BroadcastData;
use crate::spawn::actor::{
    EvtRcv, InternalHandlerResult, KSpace, MaybeDelegate, OpHashList, VecMXM,
};
use crate::spawn::meta_net::MetaNetCon;
use crate::spawn::{Internal, InternalHandler};
use crate::{GossipModuleType, KitsuneP2pError};
use futures::FutureExt;
use ghost_actor::GhostError;
use ghost_actor::{GhostControlHandler, GhostHandler};
use kitsune_p2p_fetch::{FetchContext, FetchKey, FetchSource};
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::KOpHash;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct InternalStub {
    fetch_calls: Vec<(FetchKey, KSpace, FetchSource)>,
    pub incoming_publish_calls: Arc<
        RwLock<
            Vec<(
                KSpace,
                crate::spawn::actor::KAgent,
                crate::spawn::actor::KAgent,
                OpHashList,
                FetchContext,
                MaybeDelegate,
            )>,
        >,
    >,
    pub incoming_delegate_broadcast_calls: Arc<
        RwLock<
            Vec<(
                crate::spawn::actor::KSpace,
                crate::spawn::actor::KBasis,
                crate::spawn::actor::KAgent,
                u32,
                u32,
                BroadcastData,
            )>,
        >,
    >,
    pub incoming_gossip_calls: Arc<
        RwLock<
            Vec<(
                crate::spawn::actor::KSpace,
                MetaNetCon,
                String,
                crate::spawn::actor::Payload,
                GossipModuleType,
            )>,
        >,
    >,
    pub connections: Arc<RwLock<HashMap<String, MetaNetCon>>>,
    pub respond_with_error_count: Arc<AtomicUsize>,
    pub respond_with_error: Arc<AtomicBool>,
}

impl InternalStub {
    pub fn new() -> Self {
        InternalStub {
            fetch_calls: vec![],
            incoming_publish_calls: Arc::new(RwLock::new(vec![])),
            incoming_delegate_broadcast_calls: Arc::new(RwLock::new(vec![])),
            incoming_gossip_calls: Arc::new(RwLock::new(vec![])),
            connections: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            respond_with_error_count: Arc::new(AtomicUsize::new(0)),
            respond_with_error: Arc::new(AtomicBool::new(false)),
        }
    }

    fn maybe_error(&mut self) -> Result<(), KitsuneP2pError> {
        if let Ok(true) = self.respond_with_error.compare_exchange(
            true,
            false,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            self.respond_with_error_count.fetch_add(1, Ordering::SeqCst);
            return Err("test error".into());
        }

        Ok(())
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
        space: crate::spawn::actor::KSpace,
        basis: crate::spawn::actor::KBasis,
        to_agent: crate::spawn::actor::KAgent,
        mod_idx: u32,
        mod_cnt: u32,
        data: BroadcastData,
    ) -> InternalHandlerResult<()> {
        if let Err(e) = self.maybe_error() {
            return Ok(async move { Err(e) }.boxed().into());
        }

        self.incoming_delegate_broadcast_calls
            .write()
            .push((space, basis, to_agent, mod_idx, mod_cnt, data));

        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_incoming_publish(
        &mut self,
        space: crate::spawn::actor::KSpace,
        to_agent: crate::spawn::actor::KAgent,
        source: crate::spawn::actor::KAgent,
        op_hash_list: OpHashList,
        context: FetchContext,
        maybe_delegate: MaybeDelegate,
    ) -> InternalHandlerResult<()> {
        if let Err(e) = self.maybe_error() {
            return Ok(async move { Err(e) }.boxed().into());
        }

        self.incoming_publish_calls.write().push((
            space,
            to_agent,
            source,
            op_hash_list,
            context,
            maybe_delegate,
        ));

        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_resolve_publish_pending_delegates(
        &mut self,
        _space: crate::spawn::actor::KSpace,
        _op_hash: KOpHash,
    ) -> InternalHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_incoming_gossip(
        &mut self,
        space: crate::spawn::actor::KSpace,
        con: MetaNetCon,
        remote_url: String,
        data: crate::spawn::actor::Payload,
        module_type: GossipModuleType,
    ) -> InternalHandlerResult<()> {
        if let Err(e) = self.maybe_error() {
            return Ok(async move { Err(e) }.boxed().into());
        }

        self.incoming_gossip_calls
            .write()
            .push((space, con, remote_url, data, module_type));

        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_incoming_metric_exchange(
        &mut self,
        _space: crate::spawn::actor::KSpace,
        _msgs: VecMXM,
    ) -> InternalHandlerResult<()> {
        todo!()
    }

    fn handle_new_con(&mut self, url: String, con: MetaNetCon) -> InternalHandlerResult<()> {
        self.connections.write().insert(url, con);

        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_del_con(&mut self, url: String) -> InternalHandlerResult<()> {
        self.connections.write().remove(&url);

        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_fetch(
        &mut self,
        key: FetchKey,
        space: crate::spawn::actor::KSpace,
        source: FetchSource,
    ) -> InternalHandlerResult<()> {
        self.fetch_calls.push((key, space, source));
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get_all_local_joined_agent_infos(
        &mut self,
    ) -> InternalHandlerResult<Vec<AgentInfoSigned>> {
        todo!()
    }
}

ghost_actor::ghost_chan! {
    pub chan InternalStubTest<GhostError> {
        fn drain_fetch_calls() -> Vec<(FetchKey, crate::spawn::actor::KSpace, FetchSource)>;
    }
}

impl GhostHandler<InternalStubTest> for InternalStub {}
impl InternalStubTestHandler for InternalStub {
    fn handle_drain_fetch_calls(
        &mut self,
    ) -> InternalStubTestHandlerResult<Vec<(FetchKey, KSpace, FetchSource)>> {
        let calls = self.fetch_calls.drain(..).collect();
        Ok(async move { Ok(calls) }.boxed().into())
    }
}
