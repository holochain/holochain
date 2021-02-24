use futures::future::BoxFuture;
use parking_lot::Mutex;
use std::sync::Arc;

struct AggInner<R: 'static + Send> {
    results: Vec<R>,
    is_closed: bool,
    waker: Option<std::task::Waker>,
    logic: Vec<BoxFuture<'static, ()>>,
}

/// Handle to a task aggregator (Agg) instance.
pub struct AggHandle<R: 'static + Send>(Arc<Mutex<AggInner<R>>>);

impl<R: 'static + Send> Clone for AggHandle<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<R: 'static + Send + std::fmt::Debug> AggHandle<R> {
    /// Push a result into the task aggregator.
    pub fn push_result(&self, r: R) {
        self.0.lock().results.push(r);
    }

    /// Push new logic into the task aggregator.
    pub fn push_logic<L>(&self, l: L)
    where
        L: std::future::Future<Output = ()> + 'static + Send,
    {
        let mut inner = self.0.lock();
        inner.logic.push(futures::future::FutureExt::boxed(l));
        if let Some(w) = inner.waker.take() {
            w.wake();
        }
    }

    /// Check if this task aggregator was closed.
    pub fn is_closed(&self) -> bool {
        self.0.lock().is_closed
    }

    /// Close this task aggregator.
    pub fn close(&self) {
        let mut inner = self.0.lock();
        inner.is_closed = true;
        if let Some(w) = inner.waker.take() {
            w.wake();
        }
    }
}

/// A task aggregator.
/// Capture a handle to the aggregator.
/// Fill it the aggregator with async logic.
/// Report results to the handle in the async logic.
/// Treat the aggregator as a stream, collecting the results.
pub struct Agg<R: 'static + Send>(AggHandle<R>);

impl<R: 'static + Send> Default for Agg<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: 'static + Send> Agg<R> {
    /// Create a new task aggregator instance.
    pub fn new() -> Self {
        Self(AggHandle(Arc::new(Mutex::new(AggInner {
            results: Vec::new(),
            is_closed: false,
            waker: None,
            logic: Vec::new(),
        }))))
    }

    /// A handle to this aggregator. You can clone this.
    pub fn handle(&self) -> &AggHandle<R> {
        &self.0
    }
}

impl<R: 'static + Send + std::fmt::Debug> futures::stream::Stream for Agg<R> {
    type Item = Vec<R>;

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
        for mut task in task_list {
            let mut keep = false;
            {
                let task = &mut task;
                tokio::pin!(task);
                if let std::task::Poll::Pending = std::future::Future::poll(task, cx) {
                    keep = true;
                }
            }
            if keep {
                pending_tasks.push(task);
            }
        }

        // we've run through the logic,
        // - acquire the lock
        // - check for results
        // - if any logic was added in the mean time, trigger waker
        // - restore any logic that is still pending
        let mut results = Vec::new();
        let is_closed;
        {
            let mut inner = self.0 .0.lock();
            is_closed = inner.is_closed;

            results.append(&mut inner.results);

            if !is_closed && (results.is_empty() || !pending_tasks.is_empty()) {
                if results.is_empty() && !inner.logic.is_empty() {
                    // logic was added since we pulled it out
                    // we need to explicitly wake for running again.
                    cx.waker().wake_by_ref();
                } else {
                    inner.waker = Some(cx.waker().clone());
                }
                inner.logic.append(&mut pending_tasks);
            }
        }

        if results.is_empty() {
            if is_closed {
                std::task::Poll::Ready(None)
            } else {
                std::task::Poll::Pending
            }
        } else {
            std::task::Poll::Ready(Some(results))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic;

    #[tokio::test(threaded_scheduler)]
    async fn test_agg() {
        let mut agg = <Agg<&'static str>>::new();
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
            a.push_result("a1");
            let b = a.clone();
            a.push_logic(async move {
                b.push_result("b1");
                tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
                b.push_result("b2");
            });
            tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
            a.push_result("a2");
        });

        tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
        h.close();

        rt.await.unwrap();

        assert_eq!(4, count.load(atomic::Ordering::SeqCst));
    }
}
