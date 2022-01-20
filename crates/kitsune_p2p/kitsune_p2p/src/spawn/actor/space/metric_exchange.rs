use super::*;
use crate::wire::MetricExchangeMsg;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::dht_arc::DhtArcSet;

const EXTRAP_COV_CHECK_FREQ_MS: u128 = 1000 * 60; // update once per minute
const METRIC_EXCHANGE_FREQ_MS: u128 = 1000 * 60; // exchange once per minute

struct RemoteRef {
    con: Tx2ConHnd<wire::Wire>,
    last_sync: tokio::time::Instant,
}

pub(crate) struct MetricExchange {
    space: Arc<KitsuneSpace>,
    tuning_params: KitsuneP2pTuningParams,
    shutdown: bool,
    extrap_cov: f32,
    #[allow(dead_code)]
    metrics: MetricsSync,
    remote_refs: HashMap<TxUrl, RemoteRef>,
    arc_set: DhtArcSet,
}

impl MetricExchange {
    pub fn spawn(
        space: Arc<KitsuneSpace>,
        tuning_params: KitsuneP2pTuningParams,
        metrics: MetricsSync,
    ) -> Self {
        Self {
            space,
            tuning_params,
            shutdown: false,
            extrap_cov: 0.0,
            metrics,
            remote_refs: HashMap::new(),
            arc_set: DhtArcSet::new_full(),
        }
    }

    pub fn shutdown(&mut self) {
        self.shutdown = true;
    }

    pub fn tick(&mut self) {
        for (_, r) in self.remote_refs.iter_mut() {
            if r.last_sync.elapsed().as_millis() > METRIC_EXCHANGE_FREQ_MS {
                r.last_sync = tokio::time::Instant::now();
                let space = self.space.clone();
                let timeout = self.tuning_params.implicit_timeout();
                let con = r.con.clone();
                let extrap_cov = self.extrap_cov;
                tokio::task::spawn(async move {
                    let payload = wire::Wire::metric_exchange(
                        space,
                        vec![MetricExchangeMsg::V1UniBlast {
                            extrap_cov_f32_le: extrap_cov.to_le_bytes().to_vec().into(),
                        }],
                    );
                    let _ = con.notify(&payload, timeout).await;
                });
            }
        }
    }

    pub fn update_arcset(&mut self, arc_set: DhtArcSet) {
        self.arc_set = arc_set;
    }

    pub fn new_con(&mut self, url: TxUrl, con: Tx2ConHnd<wire::Wire>) {
        use std::collections::hash_map::Entry::*;

        match self.remote_refs.entry(url) {
            Vacant(e) => {
                e.insert(RemoteRef {
                    con,
                    last_sync: tokio::time::Instant::now()
                        .checked_sub(std::time::Duration::from_secs(60 * 60))
                        .unwrap(),
                });
            }
            Occupied(mut e) => {
                if e.get().con != con {
                    let e = e.get_mut();
                    e.con = con;
                }
            }
        }
    }

    pub fn del_con(&mut self, url: TxUrl) {
        self.remote_refs.remove(&url);
    }

    pub fn ingest_msgs(&mut self, msgs: Vec<MetricExchangeMsg>) {
        for msg in msgs {
            match msg {
                MetricExchangeMsg::V1UniBlast { extrap_cov_f32_le } => {
                    if extrap_cov_f32_le.len() != 4 {
                        continue;
                    }
                    let mut tmp = [0; 4];
                    tmp.copy_from_slice(&extrap_cov_f32_le[0..4]);
                    let extrap_cov = f32::from_le_bytes(tmp);
                    self.metrics.write().record_extrap_cov_event(extrap_cov);
                }
                MetricExchangeMsg::UnknownMessage => (),
            }
        }
    }
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
        tuning_params: KitsuneP2pTuningParams,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
        metrics: MetricsSync,
    ) -> Self {
        let out = Self(Arc::new(parking_lot::RwLock::new(MetricExchange::spawn(
            space.clone(),
            tuning_params,
            metrics.clone(),
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
                        let arc_set = mx.read().arc_set.clone();
                        last_extrap_cov = tokio::time::Instant::now();
                        if let Ok(KGenRes::PeerExtrapCov(res)) = evt_sender
                            .k_gen_req(KGenReq::PeerExtrapCov {
                                space: space.clone(),
                                dht_arc_set: arc_set,
                            })
                            .await
                        {
                            // MAYBE: ignore outliers?
                            let count = res.len() as f64;
                            let res = res.into_iter().fold(0.0, |a, x| a + x) / count;
                            mx.write().extrap_cov = res as f32;
                            metrics.write().record_extrap_cov_event(res as f32);
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
