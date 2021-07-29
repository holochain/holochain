//! Task Aggregation helper utility

use futures::future::{BoxFuture, FutureExt};
use futures::stream::futures_unordered::FuturesUnordered;
use futures::stream::StreamExt;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::Notify;

/// A "Driver" is a generic static future with no output.
pub type Driver = BoxFuture<'static, ()>;

struct TaskAggInner {
    notify: Arc<Notify>,
    driver_list: Vec<Driver>,
    n_driver: bool,
    d_count: usize,
}

/// Task Aggregation helper struct handle.
#[derive(Clone)]
pub struct TaskAgg(Arc<Mutex<TaskAggInner>>);

trait S: 'static + Send + Sync {}
impl S for TaskAgg {}

impl TaskAgg {
    /// Construct a new Task Aggregation driver and handle.
    pub fn new() -> (Driver, Self) {
        let inner = Arc::new(Mutex::new(TaskAggInner {
            notify: Arc::new(Notify::new()),
            driver_list: Vec::new(),
            n_driver: false,
            d_count: 0,
        }));

        let driver = {
            let inner = inner.clone();
            async move {
                let mut fu = FuturesUnordered::new();
                let mut driver_list = Vec::new();

                loop {
                    let cont = {
                        let mut lock = inner.lock();
                        if lock.d_count == 0 {
                            false
                        } else {
                            driver_list.append(&mut lock.driver_list);
                            if !lock.n_driver {
                                lock.n_driver = true;
                                let n = lock.notify.clone();
                                let inner = inner.clone();
                                driver_list.push(
                                    async move {
                                        n.notified().await;
                                        let mut lock = inner.lock();
                                        lock.n_driver = false;
                                    }
                                    .boxed(),
                                );
                            }
                            true
                        }
                    };

                    if cont {
                        for driver in driver_list.drain(..) {
                            fu.push(driver);
                        }
                    } else {
                        // d_count must be zero, we can break
                        break;
                    }

                    let _ = fu.next().await;
                }
            }
            .boxed()
        };

        (driver, Self(inner))
    }

    /// Push a new driver task into this aggregation construct.
    pub fn push(&self, f: Driver) {
        let inner = self.0.clone();
        let mut lock = self.0.lock();
        lock.d_count += 1;
        lock.driver_list.push(
            async move {
                f.await;
                inner.lock().d_count -= 1;
            }
            .boxed(),
        );
        lock.notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_task_agg() {
        let (s, r) = tokio::sync::oneshot::channel();

        let (driver, agg) = TaskAgg::new();

        let agg2 = agg.clone();
        agg.push(
            async move {
                agg2.push(
                    async move {
                        println!("test");
                        s.send(()).unwrap();
                    }
                    .boxed(),
                );
            }
            .boxed(),
        );

        driver.await;
        r.await.unwrap();
    }
}
