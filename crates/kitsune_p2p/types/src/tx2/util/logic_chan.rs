use crate::tx2::util::Share;
use crate::*;
use futures::future::BoxFuture;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[allow(dead_code)]
struct LogicChanInner<E: 'static + Send> {
    events: Vec<E>,
    waker: Option<std::task::Waker>,
    limit: Arc<Semaphore>,
    logic: Vec<(OwnedSemaphorePermit, BoxFuture<'static, ()>)>,
}

/// Handle to a logic_chan instance.
/// A clone of a LogicChanHandle is `Eq` to its origin.
/// A clone of a LogicChanHandle will `Hash` the same as its origin.
pub struct LogicChanHandle<E: 'static + Send>(Share<LogicChanInner<E>>);

impl<E: 'static + Send> Clone for LogicChanHandle<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: 'static + Send> PartialEq for LogicChanHandle<E> {
    fn eq(&self, oth: &Self) -> bool {
        self.0.eq(&oth.0)
    }
}

impl<E: 'static + Send> Eq for LogicChanHandle<E> {}

impl<E: 'static + Send> std::hash::Hash for LogicChanHandle<E> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<E: 'static + Send> LogicChanHandle<E> {
    /// Cause the logic_chan to emit an event.
    pub fn emit(&self, e: E) -> KitsuneResult<()> {
        let waker = self.0.share_mut(move |i, _| {
            i.events.push(e);
            Ok(i.waker.take())
        })?;
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(())
    }

    /// Capture new logic into the logic_chan.
    /// The passed future can capture other async objects such as streams,
    /// that will be polled as a part of the main logic_chan stream,
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

    /// Check if this logic_chan was closed.
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// Close this logic_chan.
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

/// A logic channel.
/// Capture a handle to the logic_chan.
/// Fill the LogicChan with async logic.
/// Report events to the handle in the async logic.
/// Treat the LogicChan as a stream, collecting the events.
pub struct LogicChan<E: 'static + Send>(LogicChanHandle<E>);

impl<E: 'static + Send> LogicChan<E> {
    /// Create a new LogicChan instance.
    pub fn new(capture_bound: usize) -> Self {
        Self(LogicChanHandle(Share::new(LogicChanInner {
            events: Vec::new(),
            waker: None,
            limit: Arc::new(Semaphore::new(capture_bound)),
            logic: Vec::with_capacity(capture_bound),
        })))
    }

    /// A handle to this logic_chan. You can clone this.
    pub fn handle(&self) -> &LogicChanHandle<E> {
        &self.0
    }
}

impl<E: 'static + Send> futures::stream::Stream for LogicChan<E> {
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
                // TODO
                // Currently we're just polling all sub futures
                // even though some/most of them may not have been woken.
                // We could store an AtomicBool with each of these,
                // passing custom context wakers that would flag these bools
                // letting us know which tasks are actually ready to be polled.
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
        // - check for events
        // - if any logic was added in the mean time, trigger waker
        // - restore any logic that is still pending
        let waker = cx.waker().clone();
        let (is_closed, event) = match self.0 .0.share_mut(move |i, _| {
            if i.events.is_empty() && !i.logic.is_empty() {
                // logic was added since we pulled it out
                // we need to explicitly wake for running again.
                waker.wake();
            } else if i.events.is_empty() {
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

        // return the appropriate poll variant
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
    async fn test_util_logic_chan() {
        let mut logic_chan = <LogicChan<&'static str>>::new(32);
        let h = logic_chan.handle().clone();
        let a = logic_chan.handle().clone();

        let count = Arc::new(atomic::AtomicUsize::new(0));

        let count2 = count.clone();
        let rt = tokio::task::spawn(async move {
            while let Some(_res) = futures::stream::StreamExt::next(&mut logic_chan).await {
                count2.fetch_add(1, atomic::Ordering::SeqCst);
            }
        });

        let wt = tokio::task::spawn(async move {
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

        wt.await.unwrap();
        rt.await.unwrap();

        assert_eq!(4, count.load(atomic::Ordering::SeqCst));
    }
}
