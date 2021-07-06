#![allow(clippy::blocks_in_if_conditions)]

use crate::tx2::tx2_utils::*;
use crate::*;

/// Sync callback signature to be invoked on notify()
type NotifySyncCb = Box<dyn FnOnce() + 'static + Send>;

struct Inner {
    wakers: Vec<std::task::Waker>,
    cbs: Vec<NotifySyncCb>,
}

type InnerWrap = Arc<Share<Inner>>;

struct WaitFut(InnerWrap, Option<usize>);

impl std::future::Future for WaitFut {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut index = self.1.take();
        if self
            .0
            .share_mut(|i, _| {
                if let Some(idx) = index {
                    i.wakers[idx] = cx.waker().clone();
                    index = Some(idx);
                } else {
                    index = Some(i.wakers.len());
                    i.wakers.push(cx.waker().clone());
                }
                Ok(())
            })
            .is_err()
        {
            return std::task::Poll::Ready(());
        }
        self.1 = index;
        std::task::Poll::Pending
    }
}

fn do_notify(inner: &InnerWrap) {
    if let Ok((wakers, cbs)) = inner.share_mut(|i, c| {
        *c = true;
        Ok((
            i.wakers.drain(..).collect::<Vec<_>>(),
            i.cbs.drain(..).collect::<Vec<_>>(),
        ))
    }) {
        for waker in wakers {
            waker.wake();
        }
        for cb in cbs {
            cb();
        }
    }
}

#[derive(Clone)]
struct NotifyOnDrop(InnerWrap);

impl Drop for NotifyOnDrop {
    fn drop(&mut self) {
        do_notify(&self.0)
    }
}

/// Many tasks can await on this notify struct.
/// They will all be notified once notify is called.
#[derive(Clone)]
pub struct NotifyAll(InnerWrap, Arc<NotifyOnDrop>);

impl Default for NotifyAll {
    fn default() -> Self {
        Self::new()
    }
}

impl NotifyAll {
    /// Construct a new NotifyAll instance.
    pub fn new() -> Self {
        let inner = Inner {
            wakers: Vec::new(),
            cbs: Vec::new(),
        };
        let wrap = Arc::new(Share::new(inner));
        let notify_on_drop = Arc::new(NotifyOnDrop(wrap.clone()));
        Self(wrap, notify_on_drop)
    }

    /// Register a sync cb to be invoked on notify
    /// (will be invoked immediately if did_notify())
    pub fn wait_cb<F>(&self, sync_cb: F)
    where
        F: FnOnce() + 'static + Send,
    {
        let mut maybe_sync_cb: Option<NotifySyncCb> = Some(Box::new(sync_cb));

        // if we have not already notified, take it to be notified later
        let _ = self.0.share_mut(|i, _| {
            i.cbs.push(maybe_sync_cb.take().unwrap());
            Ok(())
        });

        // if the cb was not taken, we have already notified, so call it now
        if let Some(sync_cb) = maybe_sync_cb {
            sync_cb();
        }
    }

    /// Wait on this NotifyAll instance.
    pub fn wait(&self) -> impl std::future::Future<Output = ()> + 'static + Send {
        WaitFut(self.0.clone(), None)
    }

    /// Trigger all waiters.
    pub fn notify(&self) {
        do_notify(&self.0)
    }

    /// Has this notify all already been triggered?
    pub fn did_notify(&self) -> bool {
        self.0.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_notify_all_on_drop() {
        let count = Arc::new(atomic::AtomicUsize::new(0));

        let t = {
            let n = NotifyAll::new();
            let c2 = count.clone();
            n.wait_cb(move || {
                c2.fetch_add(1, atomic::Ordering::Relaxed);
            });
            let c3 = count.clone();
            let not = n.wait();
            let t = metric_task(async move {
                not.await;
                c3.fetch_add(1, atomic::Ordering::Relaxed);
                KitsuneResult::Ok(())
            });

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;

            assert_eq!(0, count.load(atomic::Ordering::Relaxed));

            t
        };

        t.await.unwrap().unwrap();

        assert_eq!(2, count.load(atomic::Ordering::Relaxed));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_notify_all_sync() {
        let count = Arc::new(atomic::AtomicUsize::new(0));

        let n = NotifyAll::new();

        let c2 = count.clone();
        n.wait_cb(move || {
            c2.fetch_add(1, atomic::Ordering::Relaxed);
        });
        let c3 = count.clone();
        n.wait_cb(move || {
            c3.fetch_add(1, atomic::Ordering::Relaxed);
        });

        n.notify();

        assert_eq!(2, count.load(atomic::Ordering::Relaxed));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_notify_all() {
        let count = Arc::new(atomic::AtomicUsize::new(0));

        let n = NotifyAll::new();

        let mut all = Vec::new();
        for _ in 0..10 {
            let not = n.wait();
            let count = count.clone();
            all.push(metric_task(async move {
                not.await;
                count.fetch_add(1, atomic::Ordering::Relaxed);
                KitsuneResult::Ok(())
            }));
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        assert_eq!(0, count.load(atomic::Ordering::Relaxed));

        n.notify();

        futures::future::try_join_all(all).await.unwrap();

        assert_eq!(10, count.load(atomic::Ordering::Relaxed));
    }
}
