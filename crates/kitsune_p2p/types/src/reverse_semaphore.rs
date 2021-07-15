//! ReverseSemaphore allow waiting for all permits to be released.

use parking_lot::RwLock;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Notify;

/// When this and all other permits are dropped,
/// futures awaiting `wait_on_zero_permits` will resolve.
pub struct ReverseSemaphorePermit(Arc<RwLock<Inner>>);

/// ReverseSemaphore allow waiting for all permits to be released.
#[derive(Clone)]
pub struct ReverseSemaphore(Arc<RwLock<Inner>>);

impl Default for ReverseSemaphore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReverseSemaphore {
    /// Construct a new ReverseSemaphore
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(Inner {
            n: Arc::new(Notify::new()),
            c: 0,
        })))
    }

    /// Acquire a ReverseSemaphorePermit
    pub fn acquire(&self) -> ReverseSemaphorePermit {
        ReverseSemaphorePermit::new(self.0.clone())
    }

    /// If no permits are outstanding this future will resolve immediately.
    /// If there are permits outstanding, this future will only resolve
    /// when all outstanding permits are released.
    pub fn wait_on_zero_permits(&self) -> impl Future<Output = ()> + 'static + Send {
        let inner = self.0.clone();
        async move {
            let n;
            let fut;
            {
                let lock = inner.read();
                if lock.c == 0 {
                    // if there are already zero permits, no need to wait
                    return;
                }
                n = lock.n.clone();
                // make sure to capture this notified before dropping the lock
                fut = n.notified();
                // make sure to drop the lock before awaiting the notified fut
                drop(lock);
            }
            fut.await;
        }
    }
}

// -- private -- //

struct Inner {
    n: Arc<Notify>,
    c: usize,
}

impl ReverseSemaphorePermit {
    fn new(inner: Arc<RwLock<Inner>>) -> Self {
        inner.write().c += 1;
        Self(inner)
    }
}

impl Drop for ReverseSemaphorePermit {
    fn drop(&mut self) {
        let mut lock = self.0.write();
        lock.c -= 1;
        if lock.c == 0 {
            // alas, we need to maintain the lock while we notify,
            // otherwise a new permit could be acquired and be notified
            // erroneously.
            lock.n.notify_waiters();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_reverse_semaphore() {
        let rs = ReverseSemaphore::new();

        let s = std::time::Instant::now();
        rs.wait_on_zero_permits().await;
        let es = s.elapsed().as_secs_f64();
        assert!(es < 0.00015);
        println!("zero wait, after {} s", es);

        let s = std::time::Instant::now();
        for t in 10..15 {
            let permit = rs.acquire();
            tokio::task::spawn(async move {
                let _permit = permit;
                tokio::time::sleep(std::time::Duration::from_millis(t)).await;
            });
        }
        rs.wait_on_zero_permits().await;
        let es = s.elapsed().as_secs_f64();
        assert!(es > 0.00015);
        println!("permit wait, after {} s", es);
    }
}
