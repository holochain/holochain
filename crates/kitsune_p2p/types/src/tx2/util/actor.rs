use futures::future::BoxFuture;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::{Semaphore, OwnedSemaphorePermit};

struct ActorInner<E: 'static + Send> {
    events: Vec<E>,
    is_closed: bool,
    waker: Option<std::task::Waker>,
    limit: Arc<Semaphore>,
    logic: Vec<(OwnedSemaphorePermit, BoxFuture<'static, ()>)>,
}

/// Handle to an actor instance.
/// A clone of an ActorHandle is `Eq` to its origin.
/// A clone of an ActorHandle will `Hash` the same as its origin.
pub struct ActorHandle<E: 'static + Send>(Arc<Mutex<ActorInner<E>>>);

impl<E: 'static + Send> Clone for ActorHandle<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: 'static + Send> PartialEq for ActorHandle<E> {
    fn eq(&self, oth: &Self) -> bool {
        Arc::ptr_eq(&self.0, &oth.0)
    }
}

impl<E: 'static + Send> Eq for ActorHandle<E> {}

impl<E: 'static + Send> std::hash::Hash for ActorHandle<E> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl<E: 'static + Send + std::fmt::Debug> ActorHandle<E> {
    /// Cause the actor to emit an event.
    pub fn emit(&self, e: E) {
        self.0.lock().events.push(e);
    }

    /// Capture new logic into the actor.
    /// The passed future can capture other async objects such as streams,
    /// that will be polled as a part of the main actor stream,
    /// without introducing any executor tasks.
    /// Be careful calling `capture_logic()` from within previously captured
    /// logic. While there may be reason to do this, it can lead to
    /// deadlock when approaching the capture_bound.
    pub fn capture_logic<L>(&self, l: L) -> impl std::future::Future<Output = ()> + 'static + Send
    where
        L: std::future::Future<Output = ()> + 'static + Send,
    {
        let inner = self.0.clone();
        async move {
            let permit = {
                let limit = {
                    inner.lock().limit.clone()
                };
                limit.acquire_owned()
            }.await;

            let mut inner = inner.lock();
            inner.logic.push((permit, futures::future::FutureExt::boxed(l)));
            if let Some(w) = inner.waker.take() {
                w.wake();
            }
        }
    }

    /// Check if this task actor was closed.
    pub fn is_closed(&self) -> bool {
        self.0.lock().is_closed
    }

    /// Close this task actor.
    pub fn close(&self) {
        let mut inner = self.0.lock();
        inner.is_closed = true;
        if let Some(w) = inner.waker.take() {
            w.wake();
        }
    }
}

/// A task actor.
/// Capture a handle to the actor.
/// Fill it the actor with async logic.
/// Report events to the handle in the async logic.
/// Treat the actor as a stream, collecting the events.
pub struct Actor<E: 'static + Send>(ActorHandle<E>);

impl<E: 'static + Send> Actor<E> {
    /// Create a new task actor instance.
    pub fn new(capture_bound: usize) -> Self {
        Self(ActorHandle(Arc::new(Mutex::new(ActorInner {
            events: Vec::new(),
            is_closed: false,
            waker: None,
            limit: Arc::new(Semaphore::new(capture_bound)),
            logic: Vec::with_capacity(capture_bound),
        }))))
    }

    /// A handle to this actor. You can clone this.
    pub fn handle(&self) -> &ActorHandle<E> {
        &self.0
    }
}

impl<E: 'static + Send + std::fmt::Debug> futures::stream::Stream for Actor<E> {
    type Item = Vec<E>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // tasks marked as pending - we'll need to re-queue them
        let mut pending_tasks = Vec::new();

        // do a single logic loop - if any of this logic injects more logic
        // the waker will be woken and we'll just be polled again.
        let mut task_list = Vec::new();
        {
            let mut inner = self.0 .0.lock();
            task_list.append(&mut inner.logic);
        }

        // make sure the lock is released while we execute the extracted logic:
        for (permit, mut task) in task_list {
            let mut keep = false;
            {
                let task = &mut task;
                tokio::pin!(task);
                if let std::task::Poll::Pending = std::future::Future::poll(task, cx) {
                    keep = true;
                }
            }
            if keep {
                pending_tasks.push((permit, task));
            }
        }

        // we've run through the logic,
        // - acquire the lock
        // - check for events
        // - if any logic was added in the mean time, trigger waker
        // - restore any logic that is still pending
        let mut events = Vec::new();
        let is_closed;
        {
            let mut inner = self.0 .0.lock();
            is_closed = inner.is_closed;

            events.append(&mut inner.events);

            if !is_closed && (events.is_empty() || !pending_tasks.is_empty()) {
                if events.is_empty() && !inner.logic.is_empty() {
                    // logic was added since we pulled it out
                    // we need to explicitly wake for running again.
                    cx.waker().wake_by_ref();
                } else {
                    inner.waker = Some(cx.waker().clone());
                }
                inner.logic.append(&mut pending_tasks);
            }
        }

        if events.is_empty() {
            if is_closed {
                std::task::Poll::Ready(None)
            } else {
                std::task::Poll::Pending
            }
        } else {
            std::task::Poll::Ready(Some(events))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic;

    #[tokio::test(threaded_scheduler)]
    async fn test_agg() {
        let mut agg = <Actor<&'static str>>::new(32);
        let h = agg.handle().clone();
        let a = agg.handle().clone();

        let count = Arc::new(atomic::AtomicUsize::new(0));

        let count2 = count.clone();
        let rt = tokio::task::spawn(async move {
            while let Some(res) = futures::stream::StreamExt::next(&mut agg).await {
                for _ in res {
                    count2.fetch_add(1, atomic::Ordering::SeqCst);
                }
            }
        });

        tokio::task::spawn(async move {
            a.emit("a1");
            let b = a.clone();
            a.capture_logic(async move {
                b.emit("b1");
                tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
                b.emit("b2");
            }).await;
            tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
            a.emit("a2");
        });

        tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
        h.close();

        rt.await.unwrap();

        assert_eq!(4, count.load(atomic::Ordering::SeqCst));
    }
}
