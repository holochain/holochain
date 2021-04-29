use crate::agent_store::AgentInfoSigned;
use crate::types::gossip::*;
use crate::types::*;
use futures::future::{BoxFuture, FutureExt};
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
#[allow(dead_code)]
const MAX_SEND_BUF_BYTES: usize = 16000;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MetaOpKey {
    /// data key type
    Op(Arc<KitsuneOpHash>),

    /// agent key type
    Agent(Arc<KitsuneAgent>),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MetaOpData {
    /// data chunk type
    Op(Arc<KitsuneOpHash>, Vec<u8>),

    /// agent chunk type
    Agent(AgentInfoSigned),
}

type KeySet = HashSet<Arc<MetaOpKey>>;
type HasMap = HashMap<Arc<KitsuneAgent>, KeySet>;
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

mod sparse_data_map;
use sparse_data_map::*;
mod sync_local_agents;
use sync_local_agents::*;
mod step_2_local_sync_inner;
use step_2_local_sync_inner::*;
mod step_3_initiate_inner;
use step_3_initiate_inner::*;
mod step_4_com_loop_inner;
use step_4_com_loop_inner::*;

kitsune_p2p_types::write_codec_enum! {
    /// SimpleBloom Gossip Wire Protocol Codec
    codec GossipWire {
        /// Initiate a round of gossip with a remote node
        Initiate(0x10) {
            filter.0: PoolBuf,
        },

        /// Accept an incoming round of gossip from a remote node
        Accept(0x20) {
            filter.0: PoolBuf,
        },

        /// Send a chunks of gossip meta op data,
        /// if "finished" this will be the final chunk.
        Chunk(0x30) {
            finished.0: bool,
            chunks.1: Vec<MetaOpData>,
        },
    }
}

#[allow(dead_code)]
struct NodeInfo {
    last_touch: std::time::Instant,
    was_err: bool,
}

pub(crate) enum HowToConnect {
    Con(Tx2ConHnd<wire::Wire>),
    Url(TxUrl),
}

#[allow(dead_code)]
pub(crate) struct SimpleBloomModInner2 {
    tuning_params: KitsuneP2pTuningParams,
    space: Arc<KitsuneSpace>,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,

    local_agents: HashSet<Arc<KitsuneAgent>>,
    local_bloom: BloomFilter,
    local_data_map: DataMap,
    local_key_set: KeySet,

    remote_metrics: HashMap<Tx2Cert, NodeInfo>,

    last_initiate_check: std::time::Instant,
    initiate_tgt: Option<Tx2Cert>,

    incoming: Vec<(Tx2ConHnd<wire::Wire>, GossipWire)>,

    last_outgoing: std::time::Instant,
    send_interval_ms: u64,
    outgoing: Vec<(Tx2Cert, HowToConnect, GossipWire)>,
}

impl SimpleBloomModInner2 {
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Self {
        let send_interval_ms: u64 = (
            // !*)&^$# cargo fmt...
            16384.0    // max bytes in a gossip msg
            * 8.0      // bits per byte
            * 1000.0   // milliseconds
            / 1024.0   // kbps
            / 1024.0   // mbps
            / tuning_params.gossip_output_target_mbps
        ) as u64;

        // pick an old instant for initialization
        let old = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(60 * 24))
            .unwrap();

        Self {
            tuning_params,
            space,
            ep_hnd,
            evt_sender,

            local_agents: HashSet::new(),
            local_bloom: bloomfilter::Bloom::new(1, 1),
            local_data_map: HashMap::new(),
            local_key_set: HashSet::new(),

            remote_metrics: HashMap::new(),

            last_initiate_check: old,
            initiate_tgt: None,

            incoming: Vec::new(),

            last_outgoing: old,
            send_interval_ms,
            outgoing: Vec::new(),
        }
    }
}

enum CheckResult {
    Close,
    NotReady,
    SyncAndInitiate,
    SkipSyncAndInitiate,
}

#[allow(dead_code)]
struct SimpleBloomMod2(Share<SimpleBloomModInner2>);

impl SimpleBloomMod2 {
    #[allow(dead_code)]
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Arc<Self> {
        let inner = SimpleBloomModInner2::new(tuning_params, space, ep_hnd, evt_sender);

        let send_interval_ms = inner.send_interval_ms;

        let this = Arc::new(Self(Share::new(inner)));

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

    async fn run_one_iteration(&self) -> GossipIterationResult {
        // # Step 1 - check state
        //   - if closed, send GossipIterationResult::Close
        //   - if not ready, exit early
        let sync_and_initiate = match self.step_1_check() {
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

        GossipIterationResult::Good
    }

    fn step_1_check(&self) -> CheckResult {
        match self.0.share_mut(|i, _| {
            if i.local_agents.is_empty() {
                return Ok(CheckResult::NotReady);
            }
            if i.initiate_tgt.is_none()
                && i.last_initiate_check.elapsed().as_millis() as u32
                    > i.tuning_params.gossip_loop_iteration_delay_ms
            {
                return Ok(CheckResult::SyncAndInitiate);
            }
            Ok(CheckResult::SkipSyncAndInitiate)
        }) {
            Err(_) => CheckResult::Close,
            Ok(r) => r,
        }
    }

    async fn step_2_local_sync(&self) -> KitsuneResult<bool> {
        let (space, evt_sender, local_agents) = self.0.share_mut(|i, _| {
            Ok((
                i.space.clone(),
                i.evt_sender.clone(),
                i.local_agents.clone(),
            ))
        })?;

        let (data_map, key_set, bloom) =
            match step_2_local_sync_inner(space, evt_sender, local_agents).await {
                Err(e) => {
                    tracing::warn!("gossip error: {:?}", e);
                    return Ok(false);
                }
                Ok(r) => r,
            };

        self.0.share_mut(move |i, _| {
            i.local_data_map = data_map;
            i.local_key_set = key_set;
            i.local_bloom = bloom;
            Ok(())
        })?;

        Ok(true)
    }

    async fn step_3_initiate(&self) -> KitsuneResult<bool> {
        self.0
            .share_mut(|i, _| danger_mutex_locked_sync_step_3_initiate_inner(i))?;

        Ok(true)
    }

    async fn step_4_com_loop(&self) -> KitsuneResult<bool> {
        let loop_start = std::time::Instant::now();

        loop {
            let (tuning_params, space, ep_hnd, mut maybe_outgoing, mut maybe_incoming) =
                self.0.share_mut(|i, _| {
                    let maybe_outgoing = if !i.outgoing.is_empty()
                        && i.last_outgoing.elapsed().as_millis() as u64 > i.send_interval_ms
                    {
                        let (cert, how, gossip) = i.outgoing.remove(0);

                        // set this to a time in the future
                        // so we don't accidentally double up if sending
                        // is slow... we'll set this more reasonably
                        // when we get a success or failure below.
                        i.last_outgoing = std::time::Instant::now()
                            .checked_add(std::time::Duration::from_millis(
                                i.tuning_params.tx2_implicit_timeout_ms as u64,
                            ))
                            .unwrap();

                        Some((cert, how, gossip))
                    } else {
                        None
                    };
                    let maybe_incoming = if !i.incoming.is_empty() {
                        Some(i.incoming.remove(0))
                    } else {
                        None
                    };
                    Ok((
                        i.tuning_params.clone(),
                        i.space.clone(),
                        i.ep_hnd.clone(),
                        maybe_outgoing,
                        maybe_incoming,
                    ))
                })?;

            let will_break = (maybe_outgoing.is_none() && maybe_incoming.is_none())
                || loop_start.elapsed().as_millis() as u32
                    > tuning_params.gossip_loop_iteration_delay_ms;

            if let Some(outgoing) = maybe_outgoing.take() {
                let (cert, how, gossip) = outgoing;
                if let Err(e) = step_4_com_loop_inner_outgoing(
                    tuning_params.clone(),
                    space.clone(),
                    ep_hnd,
                    how,
                    gossip,
                )
                .await
                {
                    tracing::warn!("failed to send outgoing: {:?} {:?}", cert, e);
                    self.0.share_mut(move |i, _| {
                        i.last_outgoing = std::time::Instant::now();
                        i.remote_metrics.insert(
                            cert,
                            NodeInfo {
                                last_touch: std::time::Instant::now(),
                                was_err: true,
                            },
                        );
                        Ok(())
                    })?;
                } else {
                    self.0.share_mut(move |i, _| {
                        i.last_outgoing = std::time::Instant::now();
                        i.remote_metrics.insert(
                            cert,
                            NodeInfo {
                                last_touch: std::time::Instant::now(),
                                was_err: false,
                            },
                        );
                        Ok(())
                    })?;
                }
            }

            if let Some(incoming) = maybe_incoming.take() {
                let (con, gossip) = incoming;
                if let Err(e) = step_4_com_loop_inner_incoming(&self.0, con, gossip).await {
                    tracing::warn!("failed to process incoming: {:?}", e);
                }
            }

            if will_break {
                break;
            }
        }

        Ok(true)
    }
}

// TODO - impl AsGossipModule
impl SimpleBloomMod2 {
    #[allow(dead_code)]
    fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()> {
        use kitsune_p2p_types::codec::*;
        let (_, gossip) = GossipWire::decode_ref(&gossip_data).map_err(KitsuneError::other)?;
        self.0.share_mut(move |i, _| {
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

    #[allow(dead_code)]
    fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        let _ = self.0.share_mut(move |i, _| {
            i.local_agents.insert(a);
            Ok(())
        });
    }

    #[allow(dead_code)]
    fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        let _ = self.0.share_mut(move |i, _| {
            i.local_agents.remove(&a);
            Ok(())
        });
    }
}

pub(crate) struct SimpleBloomModInner {
    tuning_params: KitsuneP2pTuningParams,
    space: Arc<KitsuneSpace>,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    local_agents: HashSet<Arc<KitsuneAgent>>,
    bloom: BloomFilter,
    data_map: SparseDataMap,
    key_set: KeySet,
}

impl Clone for SimpleBloomModInner {
    fn clone(&self) -> Self {
        let data_map = SparseDataMap::new(self.space.clone(), self.evt_sender.clone());
        Self {
            tuning_params: self.tuning_params.clone(),
            space: self.space.clone(),
            ep_hnd: self.ep_hnd.clone(),
            evt_sender: self.evt_sender.clone(),
            local_agents: self.local_agents.clone(),
            bloom: bloomfilter::Bloom::new(1, 1),
            data_map,
            key_set: HashSet::new(),
        }
    }
}

impl SimpleBloomModInner {
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Self {
        let data_map = SparseDataMap::new(space.clone(), evt_sender.clone());
        Self {
            tuning_params,
            space,
            ep_hnd,
            evt_sender,
            local_agents: HashSet::new(),
            bloom: bloomfilter::Bloom::new(1, 1),
            data_map,
            key_set: HashSet::new(),
        }
    }
}

enum GossipIterationResult {
    Close,
    Good,
}

struct SimpleBloomMod(Share<SimpleBloomModInner>);

impl SimpleBloomMod {
    fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Self {
        let inner = SimpleBloomModInner::new(tuning_params, space, ep_hnd, evt_sender);
        SimpleBloomMod(Share::new(inner))
    }

    fn clone_inner(&self) -> KitsuneResult<SimpleBloomModInner> {
        self.0.share_mut(|i, _| Ok(i.clone()))
    }

    async fn run_one_iteration(&self) -> KitsuneResult<GossipIterationResult> {
        let inner = match self.clone_inner() {
            Err(_) => return Ok(GossipIterationResult::Close),
            Ok(i) => i,
        };

        let (data_map, key_set, bloom) = SyncLocalAgents::exec(inner).await?;

        self.0.share_mut(move |i, _| {
            i.bloom = bloom;
            i.data_map = data_map;
            i.key_set = key_set;
            Ok(())
        })?;

        Ok(GossipIterationResult::Good)
    }
}

impl AsGossipModule for SimpleBloomMod {
    // TODO FIXME - This is slowing our event processing loop...
    //              Find a way to run the actual processing in the gossip task
    fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        gossip_data: Box<[u8]>,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        use kitsune_p2p_types::codec::*;
        let inner = self.0.clone();
        async move {
            let (_, gossip) = GossipWire::decode_ref(&gossip_data).map_err(KitsuneError::other)?;

            let (key_set, remote_filter) = match gossip {
                GossipWire::Initiate(Initiate { filter }) => {
                    let (tuning_params, key_set, _ep_hnd, space, local_filter) =
                        inner.share_mut(|i, _| {
                            let local_filter = encode_bloom_filter(&i.bloom);
                            Ok((
                                i.tuning_params.clone(),
                                i.key_set.clone(),
                                i.ep_hnd.clone(),
                                i.space.clone(),
                                local_filter,
                            ))
                        })?;
                    let gossip = GossipWire::accept(local_filter);
                    let gossip = gossip.encode_vec().map_err(KitsuneError::other)?;
                    let gossip = wire::Wire::gossip(space, wire::WireData(gossip));
                    con.notify(&gossip, tuning_params.implicit_timeout())
                        .await?;
                    (key_set, decode_bloom_filter(&filter))
                }
                GossipWire::Accept(Accept { filter }) => {
                    let key_set = inner.share_mut(|i, _| Ok(i.key_set.clone()))?;
                    (key_set, decode_bloom_filter(&filter))
                }
                GossipWire::Chunk(Chunk {
                    finished: _,
                    chunks,
                }) => {
                    let chunks = chunks
                        .into_iter()
                        .map(|chunk| {
                            let key = match &chunk {
                                MetaOpData::Op(key, _) => MetaOpKey::Op(key.clone()),
                                MetaOpData::Agent(s) => {
                                    MetaOpKey::Agent(Arc::new(s.as_agent_ref().clone()))
                                }
                            };
                            (Arc::new(key), Arc::new(chunk))
                        })
                        .collect::<Vec<_>>();
                    let (space, evt_sender, local_agents) = inner.share_mut(|i, _| {
                        // go ahead and mark these as received in the filter
                        // even if we get an error accepting,
                        // the filter will be reset next local sync.
                        for (key, data) in chunks.iter() {
                            i.bloom.set(key);
                            i.data_map.inject_meta(key.clone(), data.clone());
                        }
                        Ok((
                            i.space.clone(),
                            i.evt_sender.clone(),
                            i.local_agents.clone(),
                        ))
                    })?;
                    use crate::types::event::*;
                    for agent in local_agents {
                        for (_, data) in chunks.iter() {
                            match &**data {
                                MetaOpData::Op(key, data) => {
                                    evt_sender
                                        .gossip(
                                            space.clone(),
                                            agent.clone(),
                                            agent.clone(), // TODO - from agent??
                                            key.clone(),
                                            data.clone(),
                                        )
                                        .await
                                        .map_err(KitsuneError::other)?
                                }
                                MetaOpData::Agent(agent_info_signed) => {
                                    // TODO - we only need to do this
                                    //        for one single local agent
                                    evt_sender
                                        .put_agent_info_signed(PutAgentInfoSignedEvt {
                                            space: space.clone(),
                                            agent: agent.clone(),
                                            agent_info_signed: agent_info_signed.clone(),
                                        })
                                        .await
                                        .map_err(KitsuneError::other)?
                                }
                            }
                        }
                    }
                    return Ok(());
                }
            };

            for key in key_set {
                if !remote_filter.check(&key) {
                    println!("ENENDO");
                }
            }

            Ok(())
        }
        .boxed()
    }

    fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        let _ = self.0.share_mut(move |i, _| {
            i.local_agents.insert(a);
            Ok(())
        });
    }

    fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        let _ = self.0.share_mut(move |i, _| {
            i.local_agents.remove(&a);
            Ok(())
        });
    }
}

async fn gossip_loop(
    tuning_params: KitsuneP2pTuningParams,
    gossip: Arc<SimpleBloomMod>,
) -> KitsuneResult<()> {
    loop {
        match gossip.run_one_iteration().await {
            Err(e) => {
                tracing::warn!("gossip loop iteration error: {:?}", e);
            }
            Ok(GossipIterationResult::Close) => {
                tracing::warn!("aborting gossip loop");
                break;
            }
            Ok(GossipIterationResult::Good) => (),
        }

        tokio::time::sleep(std::time::Duration::from_millis(
            tuning_params.gossip_loop_iteration_delay_ms as u64,
        ))
        .await;
    }

    Ok(())
}

struct SimpleBloomModFact;

impl AsGossipModuleFactory for SimpleBloomModFact {
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> GossipModule {
        let gossip: Arc<SimpleBloomMod> = Arc::new(SimpleBloomMod::new(
            tuning_params.clone(),
            space,
            ep_hnd,
            evt_sender,
        ));

        metric_task(gossip_loop(tuning_params, gossip.clone()));

        GossipModule(gossip)
    }
}

pub fn factory() -> GossipModuleFactory {
    GossipModuleFactory(Arc::new(SimpleBloomModFact))
}
