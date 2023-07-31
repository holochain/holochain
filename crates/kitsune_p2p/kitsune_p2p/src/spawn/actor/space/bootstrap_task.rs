use crate::event::{KitsuneP2pEvent, KitsuneP2pEventSender, PutAgentInfoSignedEvt};
use crate::spawn::actor::bootstrap::BootstrapNet;
use crate::spawn::actor::space::{SpaceInternal, SpaceInternalSender};
use crate::{KitsuneP2pResult, KitsuneSpace};
use futures::channel::mpsc::Sender;
use futures::future::BoxFuture;
use futures::FutureExt;
use ghost_actor::{GhostControlSender, GhostSender};
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::bootstrap::RandomQuery;
use parking_lot::RwLock;
use std::sync::Arc;
use url2::Url2;

pub(super) struct BootstrapTask {
    pub is_finished: bool,
}

// Trait for the bootstrap query to allow mocking in tests
trait BootstrapQuery: Send {
    fn random(&self, query: RandomQuery) -> BoxFuture<KitsuneP2pResult<Vec<AgentInfoSigned>>>;
}

struct DefaultBootstrapQuery {
    url: Option<Url2>,
    net: BootstrapNet,
}

impl BootstrapQuery for DefaultBootstrapQuery {
    fn random(&self, query: RandomQuery) -> BoxFuture<KitsuneP2pResult<Vec<AgentInfoSigned>>> {
        super::bootstrap::random(self.url.clone(), query, self.net).boxed()
    }
}

impl BootstrapTask {
    pub(super) fn spawn(
        internal_sender: GhostSender<SpaceInternal>,
        host_sender: Sender<KitsuneP2pEvent>,
        space: Arc<KitsuneSpace>,
        bootstrap_service: Option<Url2>,
        bootstrap_net: BootstrapNet,
        bootstrap_check_delay_backoff_multiplier: u32,
    ) -> Arc<RwLock<Self>> {
        let this = Arc::new(RwLock::new(BootstrapTask { is_finished: false }));
        let bootstrap_query = DefaultBootstrapQuery {
            url: bootstrap_service,
            net: bootstrap_net,
        };

        let task_this = this.clone();
        BootstrapTask::spawn_inner(
            task_this,
            internal_sender,
            host_sender,
            space,
            Box::new(bootstrap_query),
            bootstrap_check_delay_backoff_multiplier,
        )
    }

    fn spawn_inner(
        this: Arc<RwLock<Self>>,
        internal_sender: GhostSender<SpaceInternal>,
        host_sender: Sender<KitsuneP2pEvent>,
        space: Arc<KitsuneSpace>,
        bootstrap_query: Box<impl BootstrapQuery + Send + Sync + 'static>,
        bootstrap_check_delay_backoff_multiplier: u32,
    ) -> Arc<RwLock<Self>> {
        let task_this = this.clone();
        tokio::task::spawn(async move {
            const START_DELAY: std::time::Duration = std::time::Duration::from_secs(1);
            const MAX_DELAY: std::time::Duration = std::time::Duration::from_secs(60 * 60);

            let mut delay_len = START_DELAY;

            loop {
                if !internal_sender.ghost_actor_is_active() {
                    break;
                }

                tokio::time::sleep(delay_len).await;
                if delay_len <= MAX_DELAY {
                    delay_len *= bootstrap_check_delay_backoff_multiplier;
                }

                match bootstrap_query
                    .random(RandomQuery {
                        space: space.clone(),
                        limit: 8.into(),
                    })
                    .await
                {
                    Err(e) => {
                        tracing::error!(msg = "Failed to get peers from bootstrap", ?e);
                    }
                    Ok(list) => {
                        if !internal_sender.ghost_actor_is_active() {
                            break;
                        }
                        let mut peer_data = Vec::with_capacity(list.len());
                        for item in list {
                            // TODO - @neonphog someday some validation here
                            match internal_sender.is_agent_local(item.agent.clone()).await {
                                Err(err) => tracing::error!(?err),
                                Ok(is_local) => {
                                    if !is_local {
                                        // we got a result - let's add it to our store for the future
                                        peer_data.push(item);
                                    }
                                }
                            }
                        }
                        if let Err(err) = host_sender
                            .put_agent_info_signed(PutAgentInfoSignedEvt {
                                space: space.clone(),
                                peer_data,
                            })
                            .await
                        {
                            tracing::error!(?err, "error storing bootstrap agent_info");
                        }
                    }
                }
            }

            tracing::warn!("bootstrap fetch loop ending");
            task_this.write().is_finished = true;
        });

        this
    }
}

#[cfg(test)]
mod tests {
    use crate::event::{KitsuneP2pEvent, PutAgentInfoSignedEvt};
    use crate::fixt::KitsuneSpaceFixturator;
    use crate::spawn::actor::space::bootstrap_task::{BootstrapQuery, BootstrapTask};
    use crate::spawn::actor::space::DhtArc;
    use crate::spawn::actor::space::{
        KAgent, KBasis, KSpace, MaybeDelegate, OpHashList, Payload, SpaceInternal,
        SpaceInternalHandler, SpaceInternalHandlerResult, VecMXM, WireConHnd,
    };
    use crate::spawn::actor::MetaNetCon;
    use crate::types::actor::BroadcastData;
    use crate::wire::Wire;
    use crate::GossipModuleType;
    use crate::KitsuneP2pResult;
    use fixt::prelude::*;
    use futures::channel::mpsc::{channel, Receiver};
    use futures::future::BoxFuture;
    use futures::{FutureExt, StreamExt};
    use ghost_actor::actor_builder::GhostActorBuilder;
    use ghost_actor::{
        GhostControlHandler, GhostControlSender, GhostError, GhostHandler, GhostSender,
    };
    use kitsune_p2p_fetch::FetchContext;
    use kitsune_p2p_types::agent_info::AgentInfoSigned;
    use kitsune_p2p_types::bootstrap::RandomQuery;
    use kitsune_p2p_types::KOpHash;
    use parking_lot::RwLock;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn update_agent_info_from_bootstrap_server() {
        let (test_sender, mut host_receiver, _) = setup(DummySpaceInternalImpl::new()).await;

        let msg = tokio::time::timeout(Duration::from_secs(5), host_receiver.next())
            .await
            .expect("Timeout while waiting for new agents")
            .expect("Error getting new agents");

        let mut response = vec![];
        match msg {
            KitsuneP2pEvent::PutAgentInfoSigned { input, respond, .. } => {
                response.push(input);
                respond.respond(Ok(async move { Ok(()) }.boxed().into()));
            }
            _ => panic!("Unexpected message type - {:?}", msg),
        }

        let found_agents = response.get(0).unwrap();
        assert_eq!(0, found_agents.peer_data.len());

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    async fn setup(
        task: DummySpaceInternalImpl,
    ) -> (
        GhostSender<TestChan>,
        Receiver<KitsuneP2pEvent>,
        Arc<RwLock<BootstrapTask>>,
    ) {
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

        let (host_sender, host_receiver) = channel(10);

        tokio::spawn(builder.spawn(task));

        let task_config = BootstrapTask { is_finished: false };

        let space = fixt!(KitsuneSpace);
        let task = BootstrapTask::spawn_inner(
            Arc::new(RwLock::new(task_config)),
            internal_sender,
            host_sender,
            Arc::new(space),
            Box::new(TestBootstrapService {}),
            2,
        );

        (test_sender, host_receiver, task)
    }

    struct DummySpaceInternalImpl {}

    impl DummySpaceInternalImpl {
        fn new() -> Self {
            DummySpaceInternalImpl {}
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
            unreachable!()
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
            // TODO configurable
            Ok(async move { Ok(false) }.boxed().into())
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
            fn will_call() -> u32;
        }
    }

    impl GhostHandler<TestChan> for DummySpaceInternalImpl {}
    impl TestChanHandler for DummySpaceInternalImpl {
        fn handle_will_call(&mut self) -> TestChanHandlerResult<u32> {
            todo!()
        }
    }

    struct TestBootstrapService {}
    impl BootstrapQuery for TestBootstrapService {
        fn random(&self, _query: RandomQuery) -> BoxFuture<KitsuneP2pResult<Vec<AgentInfoSigned>>> {
            async move { Ok(vec![]) }.boxed().into()
        }
    }
}
