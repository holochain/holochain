use crate::tx2::util::Share;
use crate::*;
use futures::future::BoxFuture;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[allow(dead_code)]
struct ActorInner<E: 'static + Send> {
    events: Vec<E>,
    waker: Option<std::task::Waker>,
    limit: Arc<Semaphore>,
    logic: Vec<(OwnedSemaphorePermit, BoxFuture<'static, ()>)>,
}

/// Handle to an actor instance.
/// A clone of an ActorHandle is `Eq` to its origin.
/// A clone of an ActorHandle will `Hash` the same as its origin.
pub struct ActorHandle<E: 'static + Send>(Share<ActorInner<E>>);

impl<E: 'static + Send> Clone for ActorHandle<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: 'static + Send> PartialEq for ActorHandle<E> {
    fn eq(&self, oth: &Self) -> bool {
        self.0.eq(&oth.0)
    }
}

impl<E: 'static + Send> Eq for ActorHandle<E> {}

impl<E: 'static + Send> std::hash::Hash for ActorHandle<E> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<E: 'static + Send> ActorHandle<E> {
    /// Cause the actor to emit an event.
    pub fn emit(&self, e: E) -> KitsuneResult<()> {
        self.0.share_mut(move |i, _| {
            i.events.push(e);
            Ok(())
        })?;
        Ok(())
    }

    /// Capture new logic into the actor.
    /// The passed future can capture other async objects such as streams,
    /// that will be polled as a part of the main actor stream,
    /// without introducing any executor tasks.
    /// Be careful calling `capture_logic()` from within previously captured
    /// logic. While there may be reason to do this, it can lead to
    /// deadlock when approaching the capture_bound.
    pub fn capture_logic<L>(
        &self,
        l: L,
    ) -> impl std::future::Future<Output = KitsuneResult<()>> + 'static + Send
    where
        L: std::future::Future<Output = ()> + 'static + Send,
    {
        let l = futures::future::FutureExt::boxed(l);
        let inner = self.0.clone();
        async move {
            let limit = inner.share_mut(|i, _| Ok(i.limit.clone()))?;
            let permit = limit.acquire_owned().await;
            let waker = inner.share_mut(move |i, _| {
                i.logic.push((permit, l));
                Ok(i.waker.take())
            })?;
            if let Some(waker) = waker {
                waker.wake();
            }
            Ok(())
        }
    }

    /// Check if this actor was closed.
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// Close this actor.
    pub fn close(&self) {
        let maybe_waker = self.0.share_mut(|i, c| {
            *c = true;
            Ok(i.waker.take())
        });
        if let Ok(Some(waker)) = maybe_waker {
            waker.wake();
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
        Self(ActorHandle(Share::new(ActorInner {
            events: Vec::new(),
            waker: None,
            limit: Arc::new(Semaphore::new(capture_bound)),
            logic: Vec::with_capacity(capture_bound),
        })))
    }

    /// A handle to this actor. You can clone this.
    pub fn handle(&self) -> &ActorHandle<E> {
        &self.0
    }
}

impl<E: 'static + Send> futures::stream::Stream for Actor<E> {
    type Item = E;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        enum X<E: 'static + Send> {
            T(E),
            L(Vec<(OwnedSemaphorePermit, BoxFuture<'static, ()>)>),
        }

        // either pull one event to emit,
        // or the task list for processing.
        let x = match self.0 .0.share_mut(|i, _| {
            if i.events.is_empty() {
                Ok(X::L(std::mem::replace(&mut i.logic, Vec::new())))
            } else {
                Ok(X::T(i.events.remove(0)))
            }
        }) {
            Err(_) => return std::task::Poll::Ready(None),
            Ok(x) => x,
        };

        // if we got an event, return it.
        let task_list = match x {
            X::T(e) => return std::task::Poll::Ready(Some(e)),
            X::L(l) => l,
        };

        // tasks marked as pending - we'll need to re-queue them
        let mut pending_tasks = Vec::new();

        // do a single logic loop - if any of this logic injects more logic
        // the waker will be woken and we'll just be polled again.
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
        let waker = cx.waker().clone();
        let (is_closed, event) = match self.0 .0.share_mut(move |i, _| {
            if i.events.is_empty() && !i.logic.is_empty() {
                // logic was added since we pulled it out
                // we need to explicitly wake for running again.
                waker.wake_by_ref();
            } else {
                i.waker = Some(waker);
            }
            i.logic.append(&mut pending_tasks);
            if i.events.is_empty() {
                Ok(None)
            } else {
                Ok(Some(i.events.remove(0)))
            }
        }) {
            Err(_) => (true, None),
            Ok(e) => (false, e),
        };

        match event {
            None => {
                if is_closed {
                    std::task::Poll::Ready(None)
                } else {
                    std::task::Poll::Pending
                }
            }
            Some(event) => std::task::Poll::Ready(Some(event)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic;

    #[tokio::test(threaded_scheduler)]
    async fn test_util_actor() {
        let mut actor = <Actor<&'static str>>::new(32);
        let h = actor.handle().clone();
        let a = actor.handle().clone();

        let count = Arc::new(atomic::AtomicUsize::new(0));

        let count2 = count.clone();
        let rt = tokio::task::spawn(async move {
            while let Some(_res) = futures::stream::StreamExt::next(&mut actor).await {
                count2.fetch_add(1, atomic::Ordering::SeqCst);
            }
        });

        tokio::task::spawn(async move {
            a.emit("a1").unwrap();
            let b = a.clone();
            a.capture_logic(async move {
                b.emit("b1").unwrap();
                tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
                b.emit("b2").unwrap();
            })
            .await
            .unwrap();
            tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
            a.emit("a2").unwrap();
        });

        tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
        h.close();

        rt.await.unwrap();

        assert_eq!(4, count.load(atomic::Ordering::SeqCst));
    }
}
