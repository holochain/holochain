use super::*;
use crate::wire::MetricExchangeMsg;

pub(crate) struct MetricExchange {
    shutdown: bool,
    #[allow(dead_code)]
    metrics: MetricsSync,
}

impl MetricExchange {
    pub fn spawn(metrics: MetricsSync) -> Self {
        Self {
            shutdown: false,
            metrics,
        }
    }

    pub fn shutdown(&mut self) {
        self.shutdown = true;
    }

    pub fn tick(&mut self) {}

    pub fn ingest_msgs(&mut self, _msgs: Vec<MetricExchangeMsg>) {}
}

#[derive(Clone)]
pub(crate) struct MetricExchangeSync(Arc<parking_lot::RwLock<MetricExchange>>);

impl std::ops::Deref for MetricExchangeSync {
    type Target = parking_lot::RwLock<MetricExchange>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MetricExchangeSync {
    pub fn spawn(metrics: MetricsSync) -> Self {
        let out = Self(Arc::new(parking_lot::RwLock::new(MetricExchange::spawn(
            metrics,
        ))));

        {
            let mx = out.clone();
            tokio::task::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                    let mut lock = mx.write();
                    if lock.shutdown {
                        return;
                    }
                    lock.tick();
                }
            });
        }

        out
    }
}
