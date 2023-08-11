use crate::spawn::actor::{Internal, InternalSender};
use crate::HostApi;
use ghost_actor::GhostSender;
use kitsune_p2p_fetch::{FetchKey, FetchPool};

pub struct FetchTask {}

impl FetchTask {
    pub fn spawn(fetch_pool: FetchPool, host: HostApi, internal_sender: GhostSender<Internal>) {
        tokio::spawn(async move {
            loop {
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
                        tracing::debug!(?err);
                    }
                }

                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::FetchTask;
    use crate::spawn::actor::test_util::InternalStubTestSender;
    use crate::spawn::actor::{Internal, KSpace};
    use crate::spawn::test_util::{InternalStub, InternalStubTest};
    use crate::HostStub;
    use ghost_actor::actor_builder::GhostActorBuilder;
    use ghost_actor::GhostSender;
    use kitsune_p2p_fetch::test_utils::{test_req, test_source};
    use kitsune_p2p_fetch::FetchSource;
    use kitsune_p2p_fetch::{FetchKey, FetchPool};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_single_op() {
        let (fetch_pool, host_stub, internal_sender_test) = setup(InternalStub::new()).await;

        fetch_pool.push(test_req(1, None, test_source(1)));

        let fetched = wait_for_fetch_n(internal_sender_test, 1).await;

        assert_eq!(1, fetched.len());
    }

    async fn setup(
        task: InternalStub,
    ) -> (FetchPool, Arc<HostStub>, GhostSender<InternalStubTest>) {
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

        let host_stub = HostStub::new();
        FetchTask::spawn(fetch_pool.clone(), host_stub.clone(), internal_sender);

        (fetch_pool, host_stub, internal_test_sender)
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
                if all_calls.len() >= n {
                    return all_calls;
                }
            }
        })
        .await
        .unwrap()
    }
}
