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
    use crate::spawn::actor::space::test_util::SpaceInternalStub;
    use crate::spawn::actor::space::SpaceInternal;
    use futures::FutureExt;
    use ghost_actor::actor_builder::GhostActorBuilder;
    use ghost_actor::{GhostControlSender, GhostError, GhostHandler, GhostSender};
    use parking_lot::RwLock;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[tokio::test(flavor = "multi_thread")]
    async fn update_agent_info() {
        let (test_sender, _) = setup(SpaceInternalStub::new()).await;

        tokio::time::timeout(Duration::from_millis(200), async {
            loop {
                tokio::time::sleep(Duration::from_millis(1)).await;
                if test_sender.get_called_count().await.unwrap() >= 3 {
                    break;
                }
            }
        })
        .await
        .expect("Timed out before seeing 3 task runs");

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
        let (test_sender, task) = setup(SpaceInternalStub::new()).await;
        test_sender.ghost_actor_shutdown().await.unwrap();

        let max_wait = Instant::now();
        while !task.read().is_finished && max_wait.elapsed() < Duration::from_millis(100) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }

        assert!(
            task.read().is_finished,
            "Task should have been marked finished after the ghost actor shut down"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn task_stays_alive_when_update_call_errors() {
        let mut space_internal_impl = SpaceInternalStub::new();
        space_internal_impl.respond_with_error = true;
        let (test_sender, _) = setup(space_internal_impl).await;

        tokio::time::timeout(Duration::from_millis(300), async {
            loop {
                tokio::time::sleep(Duration::from_millis(1)).await;
                if test_sender.get_errored_count().await.unwrap() >= 3 {
                    break;
                }
            }
        })
        .await
        .expect("Timed out before seeing 3 errors");

        let errored_count = test_sender.get_errored_count().await.unwrap();
        assert!(
            errored_count >= 3,
            "Task should have run at least 3 times but was {}",
            errored_count
        );

        test_sender.ghost_actor_shutdown_immediate().await.unwrap();
    }

    async fn setup(
        task: SpaceInternalStub,
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

    ghost_actor::ghost_chan! {
        pub chan TestChan<GhostError> {
            fn get_called_count() -> usize;
            fn get_errored_count() -> usize;
        }
    }

    impl GhostHandler<TestChan> for SpaceInternalStub {}
    impl TestChanHandler for SpaceInternalStub {
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
