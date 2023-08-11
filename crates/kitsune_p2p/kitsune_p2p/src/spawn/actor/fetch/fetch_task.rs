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
    use crate::spawn::actor::SpaceInternal;
    use crate::spawn::test_util::SpaceInternalStub;
    use crate::HostStub;
    use ghost_actor::actor_builder::GhostActorBuilder;
    use kitsune_p2p_fetch::FetchPool;
    use tokio::sync::mpsc::channel;

    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_single_op() {
        let fetch_pool = FetchPool::new_bitwise_or();
        // FetchTask::spawn(fetch_pool, HostStub::new());
    }

    // async fn setup(task: SpaceInternalStub) {
    //     let builder = GhostActorBuilder::new();
    //
    //     let internal_sender = builder
    //         .channel_factory()
    //         .create_channel::<SpaceInternal>()
    //         .await
    //         .unwrap();
    //
    //     let (host_sender, host_receiver) = channel(10);
    //
    //     tokio::spawn(builder.spawn(task));
    // }
}
