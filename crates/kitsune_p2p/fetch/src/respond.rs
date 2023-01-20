use kitsune_p2p_types::{KOpData, KSpace};
use std::sync::Arc;

/// Drop this when response sending is complete.
pub struct FetchResponseGuard(tokio::sync::oneshot::Sender<()>);

/// Customization by code making use of the FetchResponseQueue.
pub trait FetchResponseConfig: 'static + Send + Sync {
    /// Data that is forwarded.
    type User: 'static + Send;

    /// Byte count allowed to be outstanding.
    /// Any ops requested to be enqueued over this amount
    /// will be dropped without responding.
    fn byte_limit(&self) -> u32 {
        64 * 1024 * 1024
    }

    /// Number of concurrent sends to allow.
    fn concurrent_send_limit(&self) -> u32 {
        1
    }

    /// Send this fetch response.
    fn respond(
        &self,
        space: KSpace,
        user: Self::User,
        completion_guard: FetchResponseGuard,
        op: KOpData,
    );
}

/// Manage responding to requests for data.
pub struct FetchResponseQueue<C: FetchResponseConfig> {
    byte_limit: Arc<tokio::sync::Semaphore>,
    concurrent_send_limit: Arc<tokio::sync::Semaphore>,
    config: Arc<C>,
}

impl<C: FetchResponseConfig> FetchResponseQueue<C> {
    /// Construct a new response queue.
    pub fn new(config: C) -> Self {
        let byte_limit = Arc::new(tokio::sync::Semaphore::new(config.byte_limit() as usize));
        let concurrent_send_limit = Arc::new(tokio::sync::Semaphore::new(
            config.concurrent_send_limit() as usize,
        ));
        let config = Arc::new(config);
        Self {
            byte_limit,
            concurrent_send_limit,
            config,
        }
    }

    /// Enqueue an op to be sent to a remote.
    pub fn enqueue_op(&self, space: KSpace, user: C::User, op: KOpData) {
        let len = op.size();

        if len > u32::MAX as usize {
            tracing::error!("op size > u32::MAX");
            return;
        }

        let len = len as u32;

        let byte_permit = match self.byte_limit.clone().try_acquire_many_owned(len) {
            Err(_) => {
                tracing::warn!(%len, "fetch responder overloaded, dropping op");
                return;
            }
            Ok(permit) => permit,
        };

        let c_limit = self.concurrent_send_limit.clone();
        let config = self.config.clone();
        tokio::task::spawn(async move {
            let _byte_permit = byte_permit;

            let _c_permit = match c_limit.acquire_owned().await {
                Err(_) => return,
                Ok(permit) => permit,
            };

            let (s, r) = tokio::sync::oneshot::channel();

            let guard = FetchResponseGuard(s);

            config.respond(space, user, guard, op);

            // we don't care about the response... in fact
            // it's *always* an error, because we drop it.
            let _ = r.await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kitsune_p2p_types::bin_types::{KitsuneBinType, KitsuneSpace};
    use std::sync::Mutex;

    struct TestConfInner {
        pub byte_limit: u32,
        pub concurrent_send_limit: u32,
        pub responds: Vec<(KSpace, &'static str, FetchResponseGuard, KOpData)>,
    }

    struct TestConf(Mutex<TestConfInner>);

    impl TestConf {
        pub fn new(byte_limit: u32, concurrent_send_limit: u32) -> Self {
            Self(Mutex::new(TestConfInner {
                byte_limit,
                concurrent_send_limit,
                responds: Vec::new(),
            }))
        }

        pub fn drain_responds(&self) -> Vec<(KSpace, &'static str, FetchResponseGuard, KOpData)> {
            std::mem::take(&mut self.0.lock().unwrap().responds)
        }
    }

    impl FetchResponseConfig for Arc<TestConf> {
        type User = &'static str;

        fn byte_limit(&self) -> u32 {
            self.0.lock().unwrap().byte_limit
        }

        fn concurrent_send_limit(&self) -> u32 {
            self.0.lock().unwrap().concurrent_send_limit
        }

        fn respond(&self, space: KSpace, user: Self::User, g: FetchResponseGuard, op: KOpData) {
            self.0.lock().unwrap().responds.push((space, user, g, op));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test() {
        let config = Arc::new(TestConf::new(1024, 1));

        let q = FetchResponseQueue::new(config.clone());
        assert_eq!(0, config.drain_responds().len());

        q.enqueue_op(
            Arc::new(KitsuneSpace::new(vec![0; 36])),
            "noodle",
            Arc::new(b"hello".to_vec().into()),
        );

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        assert_eq!(1, config.drain_responds().len());
    }
}
