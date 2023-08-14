use crate::spawn::actor::{Internal, InternalSender};
use crate::{HostApi, KitsuneP2pError};
use ghost_actor::{GhostError, GhostSender};
use kitsune_p2p_fetch::{FetchKey, FetchPool};
use parking_lot::RwLock;
use std::sync::Arc;

pub struct FetchTask {
    is_finished: bool,
}

impl FetchTask {
    pub fn spawn(
        fetch_pool: FetchPool,
        host: HostApi,
        internal_sender: GhostSender<Internal>,
    ) -> Arc<RwLock<Self>> {
        let this = Arc::new(RwLock::new(FetchTask { is_finished: false }));

        tokio::spawn({
            let this = this.clone();
            async move {
                'task_loop: loop {
                    let list = fetch_pool.get_items_to_fetch();

                    for (key, space, source, context) in list {
                        if let FetchKey::Op(op_hash) = &key {
                            if let Ok(mut res) = host
                                .check_op_data(space.clone(), vec![op_hash.clone()], context)
                                .await
                            {
                                if res.len() == 1 && res.remove(0) {
                                    fetch_pool.remove(&key);
                                    continue;
                                }
                            }
                        }

                        if let Err(err) = internal_sender.fetch(key, space, source).await {
                            match err {
                                KitsuneP2pError::GhostError(GhostError::Disconnected) => {
                                    tracing::warn!("Fetch task is shutting down because the internal sender is closed");
                                    break 'task_loop;
                                }
                                // TODO are these so common that we can discard them? Should there be a metric to track this?
                                _ => tracing::debug!(?err),
                            }
                        }
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }

                tracing::info!("Fetch task is finishing");
                this.write().is_finished = true;
            }
        });

        this
    }
}

#[cfg(test)]
mod tests {
    use super::FetchTask;
    use crate::spawn::actor::test_util::InternalStubTestSender;
    use crate::spawn::actor::{Internal, KSpace};
    use crate::spawn::test_util::{InternalStub, InternalStubTest};
    use crate::HostStub;
    use futures::FutureExt;
    use ghost_actor::actor_builder::GhostActorBuilder;
    use ghost_actor::{GhostControlSender, GhostSender};
    use kitsune_p2p_fetch::test_utils::{test_key_hash, test_req_op, test_req_region, test_source};
    use kitsune_p2p_fetch::FetchSource;
    use kitsune_p2p_fetch::{FetchKey, FetchPool};
    use kitsune_p2p_types::KOpHash;
    use parking_lot::{Mutex, RwLock};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn fetch_single_op() {
        let (_task, fetch_pool, internal_sender_test, held_op_data) =
            setup(InternalStub::new()).await;

        fetch_pool.push(test_req_op(1, None, test_source(1)));
        wait_for_pool_n(&fetch_pool, 1).await;

        let fetched = wait_for_fetch_n(internal_sender_test.clone(), 1).await;

        // The item should get fetched
        assert_eq!(1, fetched.len());

        // Simulate the requested item being sent back
        held_op_data.lock().insert(test_key_hash(1));

        // Move forwards by 5 minutes so that the item will be retried
        tokio::time::advance(Duration::from_secs(5 * 60)).await;

        // Then the item should be removed from the pool
        wait_for_pool_n(&fetch_pool, 0).await;

        internal_sender_test
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();
    }

    #[ignore = "open question"]
    #[tokio::test(start_paused = true)]
    async fn fetch_single_region() {
        let (_task, fetch_pool, internal_sender_test, _held_op_data) =
            setup(InternalStub::new()).await;

        fetch_pool.push(test_req_region(1, None, test_source(1)));
        wait_for_pool_n(&fetch_pool, 1).await;

        let fetched = wait_for_fetch_n(internal_sender_test.clone(), 1).await;

        assert_eq!(1, fetched.len());

        // TODO No way to mark this as fetched?

        // Move forwards by 5 minutes so that the item will be retried
        tokio::time::advance(Duration::from_secs(5 * 60)).await;

        // Never removed after fetched
        wait_for_pool_n(&fetch_pool, 0).await;

        internal_sender_test
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_task_shuts_down_if_internal_sender_closes() {
        let (task, fetch_pool, internal_sender_test, _held_op_data) =
            setup(InternalStub::new()).await;

        // Do enough testing to prove the loop is up and running
        fetch_pool.push(test_req_region(1, None, test_source(1)));
        wait_for_pool_n(&fetch_pool, 1).await;
        wait_for_fetch_n(internal_sender_test.clone(), 1).await;

        // Shut down ghost actor to close the sender
        internal_sender_test
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();

        // Move forwards by 5 minutes so that the item will be retried
        tokio::time::advance(Duration::from_secs(5 * 60)).await;

        tokio::time::timeout(Duration::from_secs(5), {
            let task = task.clone();
            async move {
                loop {
                    if task.read().is_finished {
                        return;
                    }

                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        })
        .await
        .expect("Task should have shut down but is reporting that it is still running");

        assert!(task.read().is_finished);
    }

    async fn setup(
        task: InternalStub,
    ) -> (
        Arc<RwLock<FetchTask>>,
        FetchPool,
        GhostSender<InternalStubTest>,
        Arc<Mutex<HashSet<KOpHash>>>,
    ) {
        let builder = GhostActorBuilder::new();

        let internal_sender = builder
            .channel_factory()
            .create_channel::<Internal>()
            .await
            .unwrap();

        let internal_test_sender = builder
            .channel_factory()
            .create_channel::<InternalStubTest>()
            .await
            .unwrap();

        tokio::spawn(builder.spawn(task));

        let fetch_pool = FetchPool::new_bitwise_or();

        let op_data = Arc::new(Mutex::new(HashSet::<KOpHash>::new()));

        // TODO this logic should just be common, and the HostStub can expose a hashset instead that
        //      tests can add to as required.
        let host_stub = HostStub::with_check_op_data({
            let op_data = op_data.clone();
            Box::new(move |_space, op_hashes, _ctx| {
                let op_data = op_data.lock();

                let held_hashes = op_hashes
                    .into_iter()
                    .map(|hash| op_data.contains(&hash))
                    .collect();

                async move { Ok(held_hashes) }.boxed().into()
            })
        });
        let task = FetchTask::spawn(fetch_pool.clone(), host_stub, internal_sender);

        (task, fetch_pool, internal_test_sender, op_data)
    }

    async fn wait_for_pool_n(fetch_pool: &FetchPool, n: usize) {
        tokio::time::timeout(Duration::from_secs(1), async move {
            loop {
                if fetch_pool.len() == n {
                    return;
                }

                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect(
            format!(
                "Timeout while waiting for fetch pool to contain {} items, has {}",
                n,
                fetch_pool.len()
            )
            .as_str(),
        )
    }

    async fn wait_for_fetch_n(
        internal_sender_test: GhostSender<InternalStubTest>,
        n: usize,
    ) -> Vec<(FetchKey, KSpace, FetchSource)> {
        tokio::time::timeout(Duration::from_secs(1), async move {
            let mut all_calls = vec![];

            loop {
                let calls = internal_sender_test.drain_fetch_calls().await.unwrap();

                all_calls.extend(calls);
                if all_calls.len() == n {
                    return all_calls;
                }
            }
        })
        .await
        .unwrap()
    }
}
