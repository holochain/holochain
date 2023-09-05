use kitsune_p2p_types::{KOpData, KSpace};
use std::sync::Arc;

/// Drop this when response sending is complete.
pub struct FetchResponseGuard(tokio::sync::oneshot::Sender<()>);

#[cfg(any(test, feature = "test_utils"))]
impl FetchResponseGuard {
    /// Create a new FetchResponseGuard for testing.
    pub fn new(inner: tokio::sync::oneshot::Sender<()>) -> Self {
        Self(inner)
    }
}

/// Customization by code making use of the FetchResponseQueue.
pub trait FetchResponseConfig: 'static + Clone + Send + Sync {
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
#[derive(Clone)]
pub struct FetchResponseQueue<C: FetchResponseConfig> {
    byte_limit: Arc<tokio::sync::Semaphore>,
    concurrent_send_limit: Arc<tokio::sync::Semaphore>,
    config: Arc<C>,
    /// For testing, track the number of bytes sent.
    #[cfg(feature = "test_utils")]
    pub bytes_sent: Arc<std::sync::atomic::AtomicUsize>,
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
            #[cfg(feature = "test_utils")]
            bytes_sent: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Enqueue an op to be sent to a remote.
    pub fn enqueue_op(&self, space: KSpace, user: C::User, op: KOpData) -> bool {
        let len = op.size();

        // Don't try to take more permits than the byte_limit has.
        if len > self.config.byte_limit() as usize {
            tracing::error!(
                "op size is over configured limit {}",
                self.config.byte_limit()
            );
            return false;
        }

        let len = len as u32;

        let byte_permit = match self.byte_limit.clone().try_acquire_many_owned(len) {
            Err(_) => {
                tracing::warn!(%len, "fetch responder overloaded, dropping op");
                return false;
            }
            Ok(permit) => permit,
        };

        #[cfg(feature = "test_utils")]
        self.bytes_sent
            .fetch_add(len as usize, std::sync::atomic::Ordering::SeqCst);

        let c_limit = self.concurrent_send_limit.clone();
        let config = self.config.clone();
        tokio::task::spawn(async move {
            let _byte_permit = byte_permit;

            let _c_permit = match c_limit.acquire_owned().await {
                Err(_) => {
                    tracing::error!("Unexpected closed semaphore for concurrent_send_limit");
                    return;
                }
                Ok(permit) => permit,
            };

            let (s, r) = tokio::sync::oneshot::channel();

            let guard = FetchResponseGuard(s);

            config.respond(space, user, guard, op);

            // we don't care about the response... in fact
            // it's *always* an error, because we drop it.
            let _ = r.await;
        });

        true
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

    #[derive(Clone)]
    struct TestConf(Arc<Mutex<TestConfInner>>);

    impl TestConf {
        pub fn new(byte_limit: u32, concurrent_send_limit: u32) -> Self {
            Self(Arc::new(Mutex::new(TestConfInner {
                byte_limit,
                concurrent_send_limit,
                responds: Vec::new(),
            })))
        }

        pub fn drain_responds(&self) -> Vec<(KSpace, &'static str, FetchResponseGuard, KOpData)> {
            std::mem::take(&mut self.0.lock().unwrap().responds)
        }
    }

    impl FetchResponseConfig for TestConf {
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

    #[test]
    fn config_provides_defaults() {
        #[derive(Clone)]
        struct DefaultConf;
        impl FetchResponseConfig for DefaultConf {
            type User = ();

            fn respond(
                &self,
                _space: KSpace,
                _user: Self::User,
                _completion_guard: FetchResponseGuard,
                _op: KOpData,
            ) {
                unreachable!()
            }
        }

        let config = DefaultConf;
        assert!(config.byte_limit() > 0);
        assert!(config.concurrent_send_limit() > 0);
    }

    #[test]
    fn queue_uses_input_config() {
        let config = TestConf::new(1024, 1);
        let queue = FetchResponseQueue::new(config.clone());

        // Check that the queue config is based on the input config.
        assert_eq!(
            config.byte_limit(),
            queue.byte_limit.available_permits() as u32
        );
        assert_eq!(
            config.concurrent_send_limit(),
            queue.concurrent_send_limit.available_permits() as u32
        );

        // Check that updating the input config DOES NOT update the queue config.
        // TODO They may as well be properties rather than functions
        config.0.lock().unwrap().byte_limit = 1;
        config.0.lock().unwrap().concurrent_send_limit = 2;

        assert_ne!(
            config.byte_limit(),
            queue.byte_limit.available_permits() as u32
        );
        assert_ne!(
            config.concurrent_send_limit(),
            queue.concurrent_send_limit.available_permits() as u32
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enqueue_op_single() {
        let config = TestConf::new(1024, 1);

        let q = FetchResponseQueue::new(config.clone());
        assert_eq!(0, config.drain_responds().len());

        assert!(q.enqueue_op(
            Arc::new(KitsuneSpace::new(vec![0; 36])),
            "noodle",
            Arc::new(b"hello".to_vec().into()),
        ));

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        assert_eq!(1, config.drain_responds().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enqueue_op_drops_large_op() {
        let config = TestConf::new(1024, 1);
        let q = FetchResponseQueue::new(config.clone());

        assert!(!q.enqueue_op(
            Arc::new(KitsuneSpace::new(vec![0; 36])),
            "lots-of-bytes",
            Arc::new([0; 1040].to_vec().into()),
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enqueue_op_with_insufficient_capacity_remaining() {
        let config = TestConf::new(1024, 1);
        let q = FetchResponseQueue::new(config.clone());

        assert!(q.enqueue_op(
            Arc::new(KitsuneSpace::new(vec![0; 36])),
            "lots-of-bytes",
            Arc::new([0; 1000].to_vec().into()),
        ));

        assert!(!q.enqueue_op(
            Arc::new(KitsuneSpace::new(vec![0; 36])),
            "lots-of-bytes",
            Arc::new([0; 100].to_vec().into()),
        ));
    }

    // TODO This situation is never communicated back to the caller because `enqueue_op` is effectively fire and forget
    //      but it is actually a fatal condition.
    #[tokio::test(flavor = "multi_thread")]
    async fn handles_closed_semaphore() {
        let config = TestConf::new(1024, 1);
        let q = FetchResponseQueue::new(config.clone());

        assert!(q.enqueue_op(
            Arc::new(KitsuneSpace::new(vec![0; 36])),
            "lots-of-bytes",
            Arc::new([0; 100].to_vec().into()),
        ));

        // Give the op time to queue
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        q.concurrent_send_limit.close();

        // The semaphore is closed but we only find that out inside the inner task so the enqueue should succeed.
        assert!(q.enqueue_op(
            Arc::new(KitsuneSpace::new(vec![0; 36])),
            "lots-of-bytes",
            Arc::new([0; 100].to_vec().into()),
        ));

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // But there will only be one op in the queue
        assert_eq!(1, config.drain_responds().len());
    }
}
