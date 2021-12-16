use super::*;
use crate::wire::MetricExchangeMsg;
use kitsune_p2p_types::dht_arc::DhtArcSet;

const EXTRAP_COV_CHECK_FREQ_MS: u128 = 1000 * 60; // update once per minute

pub(crate) struct MetricExchange {
    shutdown: bool,
    extrap_cov: f32,
    #[allow(dead_code)]
    metrics: MetricsSync,
}

impl MetricExchange {
    pub fn spawn(metrics: MetricsSync) -> Self {
        Self {
            shutdown: false,
            extrap_cov: 0.0,
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
    pub fn spawn(
        space: Arc<KitsuneSpace>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
        metrics: MetricsSync,
    ) -> Self {
        let out = Self(Arc::new(parking_lot::RwLock::new(MetricExchange::spawn(
            metrics,
        ))));

        {
            let mx = out.clone();
            tokio::task::spawn(async move {
                let mut last_extrap_cov = tokio::time::Instant::now()
                    .checked_sub(std::time::Duration::from_secs(60 * 60))
                    .unwrap();

                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                    if last_extrap_cov.elapsed().as_millis() > EXTRAP_COV_CHECK_FREQ_MS {
                        last_extrap_cov = tokio::time::Instant::now();
                        if let Ok(KGenRes::PeerExtrapCov(res)) = evt_sender
                            .k_gen_req(KGenReq::PeerExtrapCov {
                                space: space.clone(),
                                dht_arc_set: DhtArcSet::Full, // TODO actual set
                            })
                            .await
                        {
                            // MAYBE: ignore outliers?
                            let count = res.len() as f64;
                            let res = res.into_iter().fold(0.0, |a, x| a + x) / count;
                            mx.write().extrap_cov = res as f32;
                        }
                    }

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
