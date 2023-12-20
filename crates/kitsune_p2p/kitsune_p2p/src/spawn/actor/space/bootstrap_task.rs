use crate::event::{KitsuneP2pEvent, KitsuneP2pEventSender, PutAgentInfoSignedEvt};
use crate::spawn::actor::space::{SpaceInternal, SpaceInternalSender};
use crate::{KitsuneP2pError, KitsuneP2pResult, KitsuneSpace};
use futures::channel::mpsc::Sender;
use futures::future::BoxFuture;
use futures::FutureExt;
use ghost_actor::{GhostControlSender, GhostError, GhostSender};
use kitsune_p2p_bootstrap_client::BootstrapNet;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::bootstrap::RandomQuery;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use url2::Url2;

const MAX_AGENTS_PER_QUERY: u32 = 8;

pub(super) struct BootstrapTask {
    is_finished: bool,
    current_delay: Duration,
    max_delay: Duration,
}

// Trait for the bootstrap query to allow mocking in tests
trait BootstrapService: Send {
    fn random(&self, query: RandomQuery) -> BoxFuture<KitsuneP2pResult<Vec<AgentInfoSigned>>>;
}

struct DefaultBootstrapService {
    url: Option<Url2>,
    net: BootstrapNet,
}

impl BootstrapService for DefaultBootstrapService {
    fn random(&self, query: RandomQuery) -> BoxFuture<KitsuneP2pResult<Vec<AgentInfoSigned>>> {
        async move {
            Ok(kitsune_p2p_bootstrap_client::random(self.url.clone(), query, self.net).await?)
        }.boxed()
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
        mut bootstrap_max_delay_s: u32,
    ) -> Arc<RwLock<Self>> {
        if bootstrap_max_delay_s < 60 {
            bootstrap_max_delay_s = 60;
        }

        let this = Arc::new(RwLock::new(BootstrapTask {
            is_finished: false,
            current_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(bootstrap_max_delay_s as u64),
        }));

        let bootstrap_query = DefaultBootstrapService {
            url: bootstrap_service,
            net: bootstrap_net,
        };

        BootstrapTask::spawn_inner(
            this,
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
        bootstrap_query: Box<impl BootstrapService + Send + Sync + 'static>,
        bootstrap_check_delay_backoff_multiplier: u32,
    ) -> Arc<RwLock<Self>> {
        let task_this = this.clone();
        tokio::spawn(async move {
            let backoff_multiplier = if bootstrap_check_delay_backoff_multiplier < 2 {
                tracing::warn!(
                    "Using default bootstrap backoff multiplier 2 because configured value is too low - {}",
                    bootstrap_check_delay_backoff_multiplier
                );
                2
            } else {
                bootstrap_check_delay_backoff_multiplier
            };

            let max_delay = task_this.read().max_delay;

            loop {
                if !internal_sender.ghost_actor_is_active() {
                    break;
                }

                let current_delay = task_this.read().current_delay;
                tokio::time::sleep(current_delay).await;
                if current_delay <= max_delay {
                    // Backoff but don't exceed the configured max delay
                    task_this.write().current_delay =
                        std::cmp::min(current_delay * backoff_multiplier, max_delay);
                }

                match bootstrap_query
                    .random(RandomQuery {
                        space: space.clone(),
                        limit: MAX_AGENTS_PER_QUERY.into(),
                    })
                    .await
                {
                    Err(e) => {
                        tracing::error!(msg = "Failed to get peers from bootstrap", ?e);
                    }
                    Ok(list) => {
                        if list.len() > MAX_AGENTS_PER_QUERY as usize {
                            tracing::warn!("Expected no more than {} agents from the bootstrap server but got {}", MAX_AGENTS_PER_QUERY, list.len());
                            continue;
                        }

                        if !internal_sender.ghost_actor_is_active() {
                            break;
                        }
                        let mut peer_data = Vec::with_capacity(list.len());
                        for item in list {
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
                            match err {
                                KitsuneP2pError::GhostError(GhostError::Disconnected) => {
                                    tracing::error!(?err, "Bootstrap task cannot communicate with the host, shutting down");
                                    break;
                                }
                                _ => {
                                    tracing::error!(?err, "error storing bootstrap agent_info");
                                }
                            }
                        }
                    }
                }
            }

            tracing::warn!(?space, "bootstrap fetch loop ending for space");
            task_this.write().is_finished = true;
        });

        this
    }
}

#[cfg(test)]
mod tests {
    use crate::event::PutAgentInfoSignedEvt;
    use crate::spawn::actor::space::bootstrap_task::{BootstrapService, BootstrapTask};
    use crate::spawn::actor::space::DhtArc;
    use crate::spawn::actor::space::{
        KAgent, KBasis, KSpace, MaybeDelegate, OpHashList, Payload, SpaceInternal,
        SpaceInternalHandler, SpaceInternalHandlerResult, VecMXM, WireConHnd,
    };
    use crate::spawn::actor::MetaNetCon;
    use crate::spawn::test_util::LegacyHostStub;
    use crate::types::actor::BroadcastData;
    use crate::wire::Wire;
    use crate::KitsuneP2pResult;
    use crate::{GossipModuleType, KitsuneP2pError};
    use ::fixt::prelude::*;
    use futures::channel::mpsc::channel;
    use futures::future::BoxFuture;
    use futures::{FutureExt, SinkExt, StreamExt};
    use ghost_actor::actor_builder::GhostActorBuilder;
    use ghost_actor::{GhostControlHandler, GhostControlSender, GhostHandler, GhostSender};
    use kitsune_p2p_bin_data::fixt::*;
    use kitsune_p2p_bootstrap_client::prelude::BootstrapClientError;
    use kitsune_p2p_fetch::FetchContext;
    use kitsune_p2p_types::agent_info::AgentInfoSigned;
    use kitsune_p2p_types::bootstrap::RandomQuery;
    use kitsune_p2p_types::fixt::AgentInfoSignedFixturator;
    use kitsune_p2p_types::KOpHash;
    use parking_lot::RwLock;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_relays_agent_info_from_boostrap_server_to_host() {
        let agents = vec![fixt!(AgentInfoSigned)];
        let (test_sender, mut host_stub, _) = setup(
            DummySpaceInternalImpl::new(HashSet::new()),
            agents,
            2,
            false,
        )
        .await;

        let evt = host_stub.next_event(Duration::from_secs(5)).await;

        assert_eq!(1, evt.peer_data.len());

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_shuts_down_cleanly() {
        let agents = vec![fixt!(AgentInfoSigned)];
        let (test_sender, _, task) = setup(
            DummySpaceInternalImpl::new(HashSet::new()),
            agents,
            2,
            false,
        )
        .await;

        test_sender.ghost_actor_shutdown().await.unwrap();

        tokio::time::timeout(Duration::from_secs(5), {
            let shutdown_wait_task = task.clone();
            async move {
                while !shutdown_wait_task.read().is_finished {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        })
        .await
        .ok();

        assert!(
            task.read().is_finished,
            "Task should have been marked finished after the ghost actor shut down"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_handles_bootstrap_query_errors() {
        let agents = vec![fixt!(AgentInfoSigned)];
        let (test_sender, mut host_stub, _) =
            setup(DummySpaceInternalImpl::new(HashSet::new()), agents, 2, true).await;

        let receives = Arc::new(AtomicUsize::new(0));
        tokio::time::timeout(Duration::from_secs(30), {
            let task_receives = receives.clone();
            async move {
                for _ in 0..3 {
                    host_stub.next_event(Duration::from_secs(5)).await;
                    task_receives.fetch_add(1, Ordering::SeqCst);
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(
            3,
            receives.load(Ordering::SeqCst),
            "Expected 3 calls to have succeeded"
        );

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_query_delay_increases_exponentially() {
        let agents = vec![fixt!(AgentInfoSigned)];
        let (test_sender, mut host_stub, task) = setup(
            DummySpaceInternalImpl::new(HashSet::new()),
            agents,
            3,
            false,
        )
        .await;

        let start_time = Instant::now();
        let (mut sender, receiver) = channel(3);
        tokio::time::timeout(Duration::from_secs(30), {
            async move {
                for _ in 0..3 {
                    host_stub.next_event(Duration::from_secs(5)).await;
                    sender.send(task.read().current_delay).await.unwrap();
                }
            }
        })
        .await
        .unwrap();

        let durations = receiver.map(|d| d.as_millis()).collect::<Vec<u128>>().await;

        assert_eq!(
            vec![3, 9, 10],
            durations,
            "Expected durations to increase exponentially"
        );
        assert!(
            // It's 15 not 22 because the task actually slept for 1 + 3 + 9 milliseconds and we are reading the delay
            // after it has been updated.
            start_time.elapsed() >= Duration::from_millis(15),
            "Bootstrap task should have slept for at least as long as the delay values we saw"
        );

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_prevents_sleep_disable_via_multiplier() {
        let agents = vec![fixt!(AgentInfoSigned)];
        let (test_sender, mut host_stub, task) = setup(
            DummySpaceInternalImpl::new(HashSet::new()),
            agents,
            // Set to 0 to try and force skipping the sleep
            0,
            false,
        )
        .await;

        let (mut sender, receiver) = channel(3);
        tokio::time::timeout(Duration::from_secs(30), {
            async move {
                for _ in 0..3 {
                    host_stub.next_event(Duration::from_secs(5)).await;
                    sender.send(task.read().current_delay).await.unwrap();
                }
            }
        })
        .await
        .unwrap();

        let durations = receiver.map(|d| d.as_millis()).collect::<Vec<u128>>().await;

        assert_eq!(
            vec![2, 4, 8],
            durations,
            "Expected durations to increase exponentially"
        );

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_query_delay_increase_respects_max() {
        let agents = vec![fixt!(AgentInfoSigned)];
        // Set a high delay multiplier so that a multiplication with no max check would move the delay to 1s from 1ms,
        // instead of the actual max at 10ms.
        let (test_sender, mut host_stub, task) = setup(
            DummySpaceInternalImpl::new(HashSet::new()),
            agents,
            1000,
            false,
        )
        .await;

        let (mut sender, receiver) = channel(3);
        tokio::time::timeout(Duration::from_secs(30), {
            async move {
                for _ in 0..3 {
                    host_stub.next_event(Duration::from_secs(5)).await;
                    sender.send(task.read().current_delay).await.unwrap();
                }
            }
        })
        .await
        .unwrap();

        let durations = receiver.map(|d| d.as_millis()).collect::<Vec<u128>>().await;

        assert_eq!(
            vec![10, 10, 10],
            durations,
            "Expected durations to increase exponentially"
        );

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_query_agent_limit_is_checked() {
        // The code expects a max of 8 agents, send more
        let agents = std::iter::repeat_with(|| fixt!(AgentInfoSigned))
            .take(30)
            .collect::<Vec<_>>();
        let (test_sender, mut host_stub, _) = setup(
            DummySpaceInternalImpl::new(HashSet::new()),
            agents,
            2,
            false,
        )
        .await;

        let r = host_stub.try_next_event(Duration::from_secs(1)).await;

        // The error has to be an 'elapsed' error so that means nothing was sent to the host.
        assert!(r.is_err());

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_local_agents_in_response_are_filtered() {
        // The code expects a max of 8 agents, send more
        let agents = std::iter::repeat_with(|| fixt!(AgentInfoSigned))
            .take(8)
            .collect::<Vec<_>>();

        let local_agents = agents.iter().take(3).cloned().collect::<HashSet<_>>();

        let (test_sender, mut host_stub, _) =
            setup(DummySpaceInternalImpl::new(local_agents), agents, 2, false).await;

        let evt = host_stub.next_event(Duration::from_secs(5)).await;

        assert_eq!(5, evt.peer_data.len());

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_handles_errors_sending_to_host() {
        let agents = vec![fixt!(AgentInfoSigned)];
        let (test_sender, mut host_stub, _) =
            setup(DummySpaceInternalImpl::new(HashSet::new()), agents, 2, true).await;

        // Ask the host to respond with an error on each call, then wait a while and clear the flag
        host_stub.respond_with_error.store(true, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(10)).await;
        host_stub.respond_with_error.store(false, Ordering::SeqCst);

        // Now expect to receive from the task as usual
        let receives = Arc::new(AtomicUsize::new(0));
        tokio::time::timeout(Duration::from_secs(30), {
            let task_receives = receives.clone();
            async move {
                for _ in 0..3 {
                    host_stub.next_event(Duration::from_secs(5)).await;
                    task_receives.fetch_add(1, Ordering::SeqCst);
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(
            3,
            receives.load(Ordering::SeqCst),
            "Expected 3 calls to have succeeded"
        );

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_task_shuts_down_if_host_closes() {
        let agents = vec![fixt!(AgentInfoSigned)];
        let (_, host_stub, task) = setup(
            DummySpaceInternalImpl::new(HashSet::new()),
            agents,
            2,
            false,
        )
        .await;

        // Shuts down the stub task which will cause the host receiver to drop
        host_stub.abort();

        tokio::time::timeout(Duration::from_secs(5), {
            let shutdown_wait_task = task.clone();
            async move {
                while !shutdown_wait_task.read().is_finished {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        })
        .await
        .ok();

        assert!(
            task.read().is_finished,
            "Task should have been marked finished after the host closed"
        );
    }

    async fn setup(
        task: DummySpaceInternalImpl,
        agents: Vec<AgentInfoSigned>,
        delay_multiplier: u32,
        bootstrap_every_other_call_fails: bool,
    ) -> (
        GhostSender<SpaceInternal>,
        LegacyHostStub,
        Arc<RwLock<BootstrapTask>>,
    ) {
        let builder = GhostActorBuilder::new();

        let internal_sender = builder
            .channel_factory()
            .create_channel::<SpaceInternal>()
            .await
            .unwrap();

        let (host_sender, host_receiver) = channel(10);

        tokio::spawn(builder.spawn(task));

        let task_config = BootstrapTask {
            is_finished: false,
            current_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
        };

        let space = fixt!(KitsuneSpace);
        let task = BootstrapTask::spawn_inner(
            Arc::new(RwLock::new(task_config)),
            internal_sender.clone(),
            host_sender,
            Arc::new(space),
            Box::new(TestBootstrapService::new(
                agents,
                bootstrap_every_other_call_fails,
            )),
            delay_multiplier,
        );

        let host_stub = LegacyHostStub::start(host_receiver);

        (internal_sender, host_stub, task)
    }

    struct DummySpaceInternalImpl {
        local_agents: HashSet<KAgent>,
    }

    impl DummySpaceInternalImpl {
        fn new(local_agents: HashSet<AgentInfoSigned>) -> Self {
            DummySpaceInternalImpl {
                local_agents: local_agents.into_iter().map(|a| a.agent.clone()).collect(),
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

        fn handle_is_agent_local(&mut self, agent: KAgent) -> SpaceInternalHandlerResult<bool> {
            let is_local = self.local_agents.contains(&agent);

            Ok(async move { Ok(is_local) }.boxed().into())
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

    struct TestBootstrapService {
        every_other_call_fails: bool,
        call_count: AtomicU32,
        agents: Vec<AgentInfoSigned>,
    }

    impl TestBootstrapService {
        fn new(agents: Vec<AgentInfoSigned>, every_other_call_fails: bool) -> Self {
            TestBootstrapService {
                agents,
                call_count: AtomicU32::new(0),
                every_other_call_fails,
            }
        }
    }

    impl BootstrapService for TestBootstrapService {
        fn random(&self, _query: RandomQuery) -> BoxFuture<KitsuneP2pResult<Vec<AgentInfoSigned>>> {
            let calls = self.call_count.fetch_add(1, Ordering::SeqCst);

            if self.every_other_call_fails && calls % 2 == 1 {
                return async move {
                    Err(KitsuneP2pError::Bootstrap(BootstrapClientError::Bootstrap(
                        "test error".to_string().into_boxed_str(),
                    )))
                }
                .boxed();
            }

            async move { Ok(self.agents.clone()) }.boxed()
        }
    }
}
