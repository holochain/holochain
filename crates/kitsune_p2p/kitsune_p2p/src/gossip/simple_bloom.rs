use crate::agent_store::AgentInfoSigned;
use crate::event::MetricQuery;
use crate::event::MetricQueryAnswer;
use crate::types::event::*;
use crate::types::gossip::*;
use crate::types::*;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// max send buffer size (keep it under 16384 with a little room for overhead)
/// (this is not a tuning_param because it must be coordinated
/// with the constant in PoolBuf which cannot be set at runtime)
const MAX_SEND_BUF_BYTES: usize = 16000;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MetaOpKey {
    /// data key type
    Op(Arc<KitsuneOpHash>),

    /// agent key type
    Agent(Arc<KitsuneAgent>, u64),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MetaOpData {
    /// data chunk type
    Op(Arc<KitsuneOpHash>, Vec<u8>),

    /// agent chunk type
    Agent(AgentInfoSigned),
}

impl MetaOpData {
    fn byte_count(&self) -> usize {
        match self {
            MetaOpData::Op(h, d) => (**h).len() + d.len(),
            MetaOpData::Agent(a) => {
                let h = (**a.agent).len();
                let s = (**a.signature).len();
                let d = a.encoded_bytes.len();
                h + s + d
            }
        }
    }

    fn key(&self) -> Arc<MetaOpKey> {
        let key = match self {
            MetaOpData::Op(key, _) => MetaOpKey::Op(key.clone()),
            MetaOpData::Agent(s) => MetaOpKey::Agent(s.agent.clone(), s.signed_at_ms),
        };
        Arc::new(key)
    }
}

type KeySet = HashSet<Arc<MetaOpKey>>;
type DataMap = HashMap<Arc<MetaOpKey>, Arc<MetaOpData>>;
type BloomFilter = bloomfilter::Bloom<Arc<MetaOpKey>>;

pub(crate) fn encode_bloom_filter(bloom: &BloomFilter) -> PoolBuf {
    let bitmap: Vec<u8> = bloom.bitmap();
    let bitmap_bits: u64 = bloom.number_of_bits();
    let k_num: u32 = bloom.number_of_hash_functions();
    let sip_keys = bloom.sip_keys();
    let k1: u64 = sip_keys[0].0;
    let k2: u64 = sip_keys[0].1;
    let k3: u64 = sip_keys[1].0;
    let k4: u64 = sip_keys[1].1;

    let size = bitmap.len()
        + 8 // bitmap bits
        + 4 // k_num
        + (8 * 4) // k1-4
        ;

    let mut buf = PoolBuf::new();
    buf.reserve(size);

    buf.extend_from_slice(&bitmap_bits.to_le_bytes());
    buf.extend_from_slice(&k_num.to_le_bytes());
    buf.extend_from_slice(&k1.to_le_bytes());
    buf.extend_from_slice(&k2.to_le_bytes());
    buf.extend_from_slice(&k3.to_le_bytes());
    buf.extend_from_slice(&k4.to_le_bytes());
    buf.extend_from_slice(&bitmap);

    buf
}

pub(crate) fn decode_bloom_filter(bloom: &[u8]) -> BloomFilter {
    let bitmap_bits = u64::from_le_bytes(*arrayref::array_ref![bloom, 0, 8]);
    let k_num = u32::from_le_bytes(*arrayref::array_ref![bloom, 8, 4]);
    let k1 = u64::from_le_bytes(*arrayref::array_ref![bloom, 12, 8]);
    let k2 = u64::from_le_bytes(*arrayref::array_ref![bloom, 20, 8]);
    let k3 = u64::from_le_bytes(*arrayref::array_ref![bloom, 28, 8]);
    let k4 = u64::from_le_bytes(*arrayref::array_ref![bloom, 36, 8]);
    let sip_keys = [(k1, k2), (k3, k4)];
    bloomfilter::Bloom::from_existing(&bloom[44..], bitmap_bits, k_num, sip_keys)
}

mod step_1_check_inner;
mod step_2_local_sync_inner;
mod step_3_initiate_inner;
mod step_4_com_loop_inner;

kitsune_p2p_types::write_codec_enum! {
    /// SimpleBloom Gossip Wire Protocol Codec
    codec GossipWire {
        /// Initiate a round of gossip with a remote node
        Initiate(0x10) {
            agents.0: HashSet<Arc<KitsuneAgent>>,
            filter.1: PoolBuf,
        },

        /// Accept an incoming round of gossip from a remote node
        Accept(0x20) {
            agents.0: HashSet<Arc<KitsuneAgent>>,
            filter.1: PoolBuf,
        },

        /// Send a chunks of gossip meta op data,
        /// if "finished" this will be the final chunk.
        Chunk(0x30) {
            agents.0: HashSet<Arc<KitsuneAgent>>,
            finished.1: bool,
            chunks.2: Vec<Arc<MetaOpData>>,
        },
    }
}

struct NodeInfo {
    last_touch: std::time::SystemTime,
    was_err: bool,
}

pub(crate) enum HowToConnect {
    Con(Tx2ConHnd<wire::Wire>),
    Url(TxUrl),
}

pub(crate) struct SimpleBloomModInner {
    local_agents: HashSet<Arc<KitsuneAgent>>,
    local_bloom: BloomFilter,
    local_data_map: DataMap,
    local_key_set: KeySet,

    /// Metrics to be recorded at the end of this round of gossip
    pending_metrics: Vec<(Vec<Arc<KitsuneAgent>>, NodeInfo)>,

    last_initiate_check: std::time::Instant,
    initiate_tgt: Option<GossipTgt>,

    incoming: Vec<(Tx2ConHnd<wire::Wire>, GossipWire)>,

    last_outgoing: std::time::Instant,
    outgoing: Vec<(GossipTgt, HowToConnect, GossipWire)>,
}

impl SimpleBloomModInner {
    pub fn new() -> Self {
        // pick an old instant for initialization
        let old = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(60 * 60 * 24))
            .unwrap();

        Self {
            local_agents: HashSet::new(),
            local_bloom: bloomfilter::Bloom::new(1, 1),
            local_data_map: HashMap::new(),
            local_key_set: HashSet::new(),

            pending_metrics: Vec::new(),

            last_initiate_check: old,
            initiate_tgt: None,

            incoming: Vec::new(),

            last_outgoing: old,
            outgoing: Vec::new(),
        }
    }

    /// Record a metric to be recorded at the end of this gossip round
    // TODO: remove NodeInfo
    fn record_pending_metric(&mut self, agents: Vec<Arc<KitsuneAgent>>, was_err: bool) {
        let info = NodeInfo {
            last_touch: std::time::SystemTime::now(),
            was_err,
        };
        self.pending_metrics.push((agents, info))
    }
}

enum GossipIterationResult {
    Close,
    Good,
}

enum CheckResult {
    Close,
    NotReady,
    SyncAndInitiate,
    SkipSyncAndInitiate,
}

pub(crate) struct SimpleBloomMod {
    tuning_params: KitsuneP2pTuningParams,
    send_interval_ms: u64,
    space: Arc<KitsuneSpace>,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    inner: Share<SimpleBloomModInner>,
}

impl SimpleBloomMod {
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Arc<Self> {
        let inner = SimpleBloomModInner::new();

        let send_interval_ms: u64 = (
            // !*)&^$# cargo fmt...
            16384.0    // max bytes in a gossip msg
            * 8.0      // bits per byte
            * 1000.0   // milliseconds
            / 1024.0   // kbps
            / 1024.0   // mbps
            / tuning_params.gossip_output_target_mbps
        ) as u64;

        let this = Arc::new(Self {
            tuning_params,
            space,
            ep_hnd,
            send_interval_ms,
            evt_sender,
            inner: Share::new(inner),
        });

        // this value needs to be somewhat frequent to support send timing
        let loop_check_interval_ms = std::cmp::max(send_interval_ms / 3, 100);

        let gossip = this.clone();
        metric_task(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(
                    loop_check_interval_ms as u64,
                ))
                .await;

                if let GossipIterationResult::Close = gossip.run_one_iteration().await {
                    tracing::warn!("gossip loop ending");
                    break;
                }
            }

            KitsuneResult::Ok(())
        });

        this
    }

    /// Get metrics data via event channel in the form of NodeInfo
    // TODO: remove NodeInfo
    async fn get_metric_info(
        &self,
        agents: Vec<Arc<KitsuneAgent>>,
    ) -> KitsuneP2pResult<Option<NodeInfo>> {
        // We pick an arbitrary agent for now, since in a full-sync situation,
        // any agent should have the same data as any other agent.
        // TODO: this will naturally change after sharding.
        let arbitrary_agent = agents
            .first()
            .expect("Gossip must have a least one from_agent")
            .clone();
        let last_touch = match self
            .evt_sender
            .query_metrics(MetricQuery::LastSync {
                agent: arbitrary_agent,
            })
            .await?
        {
            MetricQueryAnswer::LastSync(time) => time,
            _ => unreachable!(),
        };
        Ok(last_touch.map(|last_touch| NodeInfo {
            last_touch,
            was_err: false,
        }))
    }

    /// Record a metric via event channel
    // TODO: remove NodeInfo
    async fn record_metric(
        &self,
        agents: Vec<Arc<KitsuneAgent>>,
        info: NodeInfo,
    ) -> KitsuneP2pResult<()> {
        if info.was_err {
            for agent in agents {
                self.evt_sender
                    .put_metric_datum(MetricDatum {
                        agent,
                        kind: MetricKind::ConnectError,
                        timestamp: info.last_touch,
                    })
                    .await?;
            }
        } else {
            for agent in agents {
                self.evt_sender
                    .put_metric_datum(MetricDatum {
                        agent,
                        kind: MetricKind::QuickGossip,
                        timestamp: info.last_touch,
                    })
                    .await?;
            }
        }
        Ok(())
    }

    async fn run_one_iteration(&self) -> GossipIterationResult {
        // # Step 1 - check state
        //   - if closed, send GossipIterationResult::Close
        //   - if not ready, exit early
        let sync_and_initiate = match self.step_1_check().await {
            CheckResult::Close => return GossipIterationResult::Close,
            CheckResult::NotReady => return GossipIterationResult::Good,
            CheckResult::SyncAndInitiate => true,
            CheckResult::SkipSyncAndInitiate => false,
        };

        if sync_and_initiate {
            // # Step 2 - run a local sync, updating bloom / data_map / key_set
            match self.step_2_local_sync().await {
                Err(_) => return GossipIterationResult::Close,
                Ok(false) => return GossipIterationResult::Good,
                Ok(true) => (),
            }

            // # Step 3 - check target for initiation
            //   - if we don't have a current initiation target, pick one
            //   - send the initiate message
            match self.step_3_initiate().await {
                Err(_) => return GossipIterationResult::Close,
                Ok(false) => return GossipIterationResult::Good,
                Ok(true) => (),
            }
        }

        // # Step 4 - loop on incoming/outgoing data in parallel
        //   - if processing incoming data is slow we want to keep
        //     sending outgoing data at appropriate times
        //   - if we get a "finished" chunk from our initaite target,
        //     clear the initiate target
        //   - if we get through all incoming/outgoing, move on
        //   - if we take > gossip_interval, move on
        match self.step_4_com_loop().await {
            Err(_) => return GossipIterationResult::Close,
            Ok(false) => return GossipIterationResult::Good,
            Ok(true) => (),
        }

        // # Step 5 - flush all pending metrics via KitsuneP2pEvent channel
        //   TODO: this may not be technically correct, since we may want to
        //       record metrics from previous steps even if those other steps
        //       short-circuited this iteration. Revisit.
        match self.step_5_flush_metrics().await {
            Err(_) => return GossipIterationResult::Close,
            Ok(false) => return GossipIterationResult::Good,
            Ok(true) => (),
        }

        GossipIterationResult::Good
    }

    async fn step_1_check(&self) -> CheckResult {
        match self.step_1_check_inner().await {
            Err(_) => CheckResult::Close,
            Ok(r) => r,
        }
    }

    async fn step_2_local_sync(&self) -> KitsuneResult<bool> {
        let local_agents = self.inner.share_mut(|i, _| Ok(i.local_agents.clone()))?;

        let (data_map, key_set, bloom) = match self.step_2_local_sync_inner(local_agents).await {
            Err(e) => {
                tracing::warn!("gossip error: {:?}", e);
                return Ok(false);
            }
            Ok(r) => r,
        };

        self.inner.share_mut(move |i, _| {
            i.local_data_map = data_map;
            i.local_key_set = key_set;
            i.local_bloom = bloom;
            Ok(())
        })?;

        Ok(true)
    }

    async fn step_3_initiate(&self) -> KitsuneP2pResult<bool> {
        self.step_3_initiate_inner().await?;
        Ok(true)
    }

    async fn step_4_com_loop(&self) -> KitsuneResult<bool> {
        self.step_4_com_loop_inner().await?;
        Ok(true)
    }

    async fn step_5_flush_metrics(&self) -> KitsuneP2pResult<bool> {
        let metrics: Vec<_> = self
            .inner
            .share_mut(|i, _| Ok(i.pending_metrics.drain(..).collect()))?;
        for (agents, info) in metrics {
            self.record_metric(agents, info).await?;
        }
        Ok(true)
    }
}

impl AsGossipModule for SimpleBloomMod {
    fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()> {
        use kitsune_p2p_types::codec::*;
        let (_, gossip) = GossipWire::decode_ref(&gossip_data).map_err(KitsuneError::other)?;
        self.inner.share_mut(move |i, _| {
            i.incoming.push((con, gossip));
            if i.incoming.len() > 20 {
                tracing::warn!(
                    "Overloaded with incoming gossip.. {} messages",
                    i.incoming.len()
                );
            }
            Ok(())
        })
    }

    fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        let _ = self.inner.share_mut(move |i, _| {
            i.local_agents.insert(a);
            Ok(())
        });
    }

    fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        let _ = self.inner.share_mut(move |i, _| {
            i.local_agents.remove(&a);
            Ok(())
        });
    }
}

struct SimpleBloomModFactory;

impl AsGossipModuleFactory for SimpleBloomModFactory {
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> GossipModule {
        GossipModule(SimpleBloomMod::new(
            tuning_params,
            space,
            ep_hnd,
            evt_sender,
        ))
    }
}

pub fn factory() -> GossipModuleFactory {
    GossipModuleFactory(Arc::new(SimpleBloomModFactory))
}
