#![allow(clippy::blocks_in_if_conditions)]

use crate::tx2::tx2_utils::*;
use crate::*;

type Inner = Arc<Share<Vec<std::task::Waker>>>;

struct WaitFut(Inner, Option<usize>);

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
                    i[idx] = cx.waker().clone();
                    index = Some(idx);
                } else {
                    index = Some(i.len());
                    i.push(cx.waker().clone());
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

/// Many tasks can await on this notify struct.
/// They will all be notified once notify is called.
#[derive(Clone)]
pub struct NotifyAll(Inner);

impl Default for NotifyAll {
    fn default() -> Self {
        Self::new()
    }
}

impl NotifyAll {
    /// Construct a new NotifyAll instance.
    pub fn new() -> Self {
        Self(Arc::new(Share::new(Vec::new())))
    }

    /// Wait on this NotifyAll instance.
    pub fn wait(&self) -> impl std::future::Future<Output = ()> + 'static + Send {
        WaitFut(self.0.clone(), None)
    }

    /// Trigger all waiters.
    pub fn notify(&self) {
        if let Ok(wakers) = self.0.share_mut(|i, c| {
            *c = true;
            Ok(i.drain(..).collect::<Vec<_>>())
        }) {
            for waker in wakers {
                waker.wake();
            }
        }
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

    #[tokio::test(threaded_scheduler)]
    async fn test_notify_all() {
        let count = Arc::new(atomic::AtomicUsize::new(0));

        let n = NotifyAll::new();

        let mut all = Vec::new();
        for _ in 0..10 {
            let not = n.wait();
            let count = count.clone();
            all.push(tokio::task::spawn(async move {
                not.await;
                count.fetch_add(1, atomic::Ordering::Relaxed);
            }));
        }

        tokio::time::delay_for(std::time::Duration::from_millis(10)).await;

        assert_eq!(0, count.load(atomic::Ordering::Relaxed));

        n.notify();

        futures::future::try_join_all(all).await.unwrap();

        assert_eq!(10, count.load(atomic::Ordering::Relaxed));
    }
}
