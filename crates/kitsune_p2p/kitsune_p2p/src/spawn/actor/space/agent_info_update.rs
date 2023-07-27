use crate::spawn::actor::space::{SpaceInternal, SpaceInternalSender};
use ghost_actor::{GhostControlSender, GhostSender};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

pub(super) struct AgentInfoUpdateTask {
    pub is_finished: bool,
}

impl AgentInfoUpdateTask {
    pub(super) fn spawn(
        internal_sender: GhostSender<SpaceInternal>,
        interval: Duration,
    ) -> Arc<RwLock<Self>> {
        let this = Arc::new(RwLock::new(AgentInfoUpdateTask { is_finished: false }));

        let task_this = this.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                if let Err(e) = internal_sender.update_agent_info().await {
                    if !internal_sender.ghost_actor_is_active() {
                        // Assume this task has been orphaned when the space was dropped and exit.
                        info!("AgentInfoUpdateTask will stop because the ghost actor it uses to communicate is closing");
                        break;
                    } else {
                        error!(failed_to_update_agent_info_for_space = ?e);
                    }
                }
            }

            info!("AgentInfoUpdateTask finished");
            task_this.write().is_finished = true;
        });

        this
    }
}

#[cfg(test)]
mod tests {
    use super::AgentInfoUpdateTask;
    use crate::actor::BroadcastData;
    use crate::dht_arc::DhtArc;
    use crate::event::PutAgentInfoSignedEvt;
    use crate::spawn::actor::space::{
        KAgent, KBasis, KSpace, MaybeDelegate, OpHashList, Payload, SpaceInternal,
        SpaceInternalHandler, SpaceInternalHandlerResult, VecMXM, WireConHnd,
    };
    use crate::spawn::meta_net::MetaNetCon;
    use crate::wire::Wire;
    use crate::{GossipModuleType, KitsuneP2pError};
    use futures::FutureExt;
    use ghost_actor::actor_builder::GhostActorBuilder;
    use ghost_actor::{
        GhostControlHandler, GhostControlSender, GhostError, GhostHandler, GhostSender,
    };
    use kitsune_p2p_fetch::FetchContext;
    use kitsune_p2p_types::agent_info::AgentInfoSigned;
    use kitsune_p2p_types::KOpHash;
    use parking_lot::RwLock;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[tokio::test(flavor = "multi_thread")]
    async fn update_agent_info() {
        let (test_sender, _) = setup(DummySpaceInternalImpl::new()).await;

        // It should be possible to set this as low as 4ms but the update_agent_info calls take a bit of time
        tokio::time::sleep(Duration::from_millis(6)).await;

        let called_count = test_sender.get_called_count().await.unwrap();
        assert!(
            called_count >= 3,
            "Task should have run at least 3 times but was {}",
            called_count
        );

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn task_shuts_down_cleanly() {
        let (test_sender, task) = setup(DummySpaceInternalImpl::new()).await;
        test_sender.ghost_actor_shutdown().await.unwrap();

        let max_wait = Instant::now();
        while !task.read().is_finished && max_wait.elapsed() < Duration::from_millis(10) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }

        assert!(
            task.read().is_finished,
            "Task should have been marked finished after the ghost actor shut down"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn task_stays_alive_when_update_call_errors() {
        let mut space_internal_impl = DummySpaceInternalImpl::new();
        space_internal_impl.respond_with_error = true;
        let (test_sender, _) = setup(space_internal_impl).await;

        // It should be possible to set this as low as 4ms but the update_agent_info calls take a bit of time
        tokio::time::sleep(Duration::from_millis(6)).await;

        let errored_count = test_sender.get_errored_count().await.unwrap();
        assert!(
            errored_count >= 3,
            "Task should have run at least 3 times but was {}",
            errored_count
        );

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    async fn setup(
        task: DummySpaceInternalImpl,
    ) -> (GhostSender<TestChan>, Arc<RwLock<AgentInfoUpdateTask>>) {
        let builder = GhostActorBuilder::new();

        let internal_sender = builder
            .channel_factory()
            .create_channel::<SpaceInternal>()
            .await
            .unwrap();

        let test_sender = builder
            .channel_factory()
            .create_channel::<TestChan>()
            .await
            .unwrap();

        tokio::spawn(builder.spawn(task));

        let task = AgentInfoUpdateTask::spawn(internal_sender, Duration::from_millis(1));

        (test_sender, task)
    }

    struct DummySpaceInternalImpl {
        called_count: usize,
        errored_count: usize,
        respond_with_error: bool,
    }

    impl DummySpaceInternalImpl {
        fn new() -> Self {
            DummySpaceInternalImpl {
                called_count: 0,
                errored_count: 0,
                respond_with_error: false,
            }
        }
    }

    impl GhostControlHandler for DummySpaceInternalImpl {}
    impl GhostHandler<SpaceInternal> for DummySpaceInternalImpl {}
    impl SpaceInternalHandler for DummySpaceInternalImpl {
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

        fn handle_notify(
            &mut self,
            _to_agent: KAgent,
            _data: Wire,
        ) -> SpaceInternalHandlerResult<()> {
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

        fn handle_new_con(
            &mut self,
            _url: String,
            _con: WireConHnd,
        ) -> SpaceInternalHandlerResult<()> {
            unreachable!()
        }

        fn handle_del_con(&mut self, _url: String) -> SpaceInternalHandlerResult<()> {
            unreachable!()
        }
    }

    ghost_actor::ghost_chan! {
        pub chan TestChan<GhostError> {
            fn get_called_count() -> usize;
            fn get_errored_count() -> usize;
        }
    }

    impl GhostHandler<TestChan> for DummySpaceInternalImpl {}
    impl TestChanHandler for DummySpaceInternalImpl {
        fn handle_get_called_count(&mut self) -> TestChanHandlerResult<usize> {
            let called_count = self.called_count;
            Ok(async move { Ok(called_count) }.boxed().into())
        }

        fn handle_get_errored_count(&mut self) -> TestChanHandlerResult<usize> {
            let errored_count = self.errored_count;
            Ok(async move { Ok(errored_count) }.boxed().into())
        }
    }
}
