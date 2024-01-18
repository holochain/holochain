use crate::spawn::actor::{Internal, InternalSender};
use crate::{HostApiLegacy, KitsuneP2pError};
use ghost_actor::{GhostError, GhostSender};
use kitsune_p2p_fetch::{FetchKey, FetchPool};
use kitsune_p2p_types::config::KitsuneP2pConfig;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::Instrument;

pub struct FetchTask {
    is_finished: bool,
}

impl FetchTask {
    pub fn spawn(
        config: KitsuneP2pConfig,
        fetch_pool: FetchPool,
        host: HostApiLegacy,
        internal_sender: GhostSender<Internal>,
    ) -> Arc<RwLock<Self>> {
        let this = Arc::new(RwLock::new(FetchTask { is_finished: false }));

        let span = tracing::error_span!("FetchTask::spawn", scope = config.tracing_scope);

        tokio::spawn({
            let this = this.clone();
            async move {
                'task_loop: loop {
                    // Drop sources that aren't responding to fetch requests, and any items that have no remaining sources to fetch from.
                    fetch_pool.check_sources();

                    let list = fetch_pool.get_items_to_fetch();

                    for (key, space, source, context) in list {
                        let FetchKey::Op(op_hash) = &key;

                        if let Ok(mut res) = host
                            .check_op_data(space.clone(), vec![op_hash.clone()], context)
                            .await
                        {
                            if res.len() == 1 && res.remove(0) {
                                fetch_pool.remove(&key);
                                continue;
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
            }.instrument(span)
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
    use kitsune_p2p_fetch::test_utils::{test_key_hash, test_req_op, test_source};
    use kitsune_p2p_fetch::FetchSource;
    use kitsune_p2p_fetch::{FetchKey, FetchPool};
    use kitsune_p2p_types::KOpHash;
    use parking_lot::{Mutex, RwLock};
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn fetch_single_op() {
        let (_task, fetch_pool, internal_sender_test, held_op_data, _) =
            setup(InternalStub::new()).await;

        fetch_pool.push(test_req_op(1, None, test_source(1)));
        wait_for_pool_n(&fetch_pool, 1).await;

        let fetched = wait_for_fetch_n(internal_sender_test.clone(), 1).await;

        // The item should get fetched
        assert_eq!(1, fetched.iter().flatten().count());

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

    #[tokio::test(start_paused = true)]
    async fn fetch_task_shuts_down_if_internal_sender_closes() {
        let (task, fetch_pool, internal_sender_test, _held_op_data, _) =
            setup(InternalStub::new()).await;

        // Do enough testing to prove the loop is up and running
        fetch_pool.push(test_req_op(1, None, test_source(1)));
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

    // TODO the API supports batch queries, why not query in batch? We are pushing extra requests through a bottleneck
    #[tokio::test(start_paused = true)]
    async fn fetch_checks_op_status_one_by_one_to_host() {
        let (_task, fetch_pool, internal_sender_test, _held_op_data, check_op_data_call_count) =
            setup(InternalStub::new()).await;

        fetch_pool.push(test_req_op(1, None, test_source(1)));
        fetch_pool.push(test_req_op(2, None, test_source(2)));
        fetch_pool.push(test_req_op(3, None, test_source(3)));
        wait_for_pool_n(&fetch_pool, 3).await;

        let fetched = wait_for_fetch_n(internal_sender_test.clone(), 3).await;

        assert_eq!(3, fetched.iter().flatten().count());

        // This should be 1 if we passed all the ops to check at once.
        assert_eq!(3, check_op_data_call_count.load(Ordering::SeqCst));

        internal_sender_test
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();
    }

    async fn setup(
        task: InternalStub,
    ) -> (
        Arc<RwLock<FetchTask>>,
        FetchPool,
        GhostSender<InternalStubTest>,
        Arc<Mutex<HashSet<KOpHash>>>,
        Arc<AtomicUsize>,
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
        let check_op_data_call_count = Arc::new(AtomicUsize::new(0));

        let (dummy_sender, _) = futures::channel::mpsc::channel(10);
        // if needed, use a real stub:
        // let (host_sender, host_receiver) = channel(10);
        // let host_receiver_stub = HostReceiverStub::start(host_receiver);

        // TODO this logic should just be common, and the HostStub can expose a hashset instead that
        //      tests can add to as required.
        let host_stub = HostStub::with_check_op_data({
            let op_data = op_data.clone();
            let check_op_data_call_count = check_op_data_call_count.clone();
            Box::new(move |_space, op_hashes, _ctx| {
                check_op_data_call_count.fetch_add(1, Ordering::SeqCst);
                let op_data = op_data.lock();

                let held_hashes = op_hashes
                    .into_iter()
                    .map(|hash| op_data.contains(&hash))
                    .collect();

                async move { Ok(held_hashes) }.boxed().into()
            })
        })
        .legacy(dummy_sender);

        let task = FetchTask::spawn(
            Default::default(),
            fetch_pool.clone(),
            host_stub,
            internal_sender,
        );

        (
            task,
            fetch_pool,
            internal_test_sender,
            op_data,
            check_op_data_call_count,
        )
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
    ) -> Vec<Vec<(FetchKey, KSpace, FetchSource)>> {
        tokio::time::timeout(Duration::from_secs(1), async move {
            let mut all_calls = vec![];

            loop {
                let calls = internal_sender_test.drain_fetch_calls().await.unwrap();

                all_calls.push(calls);
                if all_calls.iter().flatten().count() == n {
                    return all_calls;
                }
            }
        })
        .await
        .unwrap()
    }
}
