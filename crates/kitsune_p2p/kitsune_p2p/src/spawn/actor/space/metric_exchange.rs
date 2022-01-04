use super::*;
use crate::wire::MetricExchangeMsg;
use kitsune_p2p_types::dht_arc::DhtArcSet;

const EXTRAP_COV_CHECK_FREQ_MS: u128 = 1000 * 60; // update once per minute

struct RemoteRef {
    agents: Vec<Arc<KitsuneAgent>>,
    con: Tx2ConHnd<wire::Wire>,
}

pub(crate) struct MetricExchange {
    shutdown: bool,
    extrap_cov: f32,
    #[allow(dead_code)]
    metrics: MetricsSync,
    remote_refs: HashMap<TxUrl, RemoteRef>,
}

impl MetricExchange {
    pub fn spawn(metrics: MetricsSync) -> Self {
        Self {
            shutdown: false,
            extrap_cov: 0.0,
            metrics,
            remote_refs: HashMap::new(),
        }
    }

    pub fn shutdown(&mut self) {
        self.shutdown = true;
    }

    pub fn tick(&mut self) {}

    pub fn new_con(&mut self, url: TxUrl, con: Tx2ConHnd<wire::Wire>) {
        use std::collections::hash_map::Entry::*;
        match self.remote_refs.entry(url) {
            Vacant(e) => {
                e.insert(RemoteRef {
                    agents: vec![],
                    con,
                });
            }
            Occupied(mut e) => {
                if e.get().con != con {
                    let e = e.get_mut();
                    e.con = con;
                    e.agents = vec![];
                }
            }
        }
    }

    pub fn del_con(&mut self, url: TxUrl) {
        self.remote_refs.remove(&url);
    }

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
