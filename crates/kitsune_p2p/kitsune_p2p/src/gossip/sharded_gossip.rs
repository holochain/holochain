//! The main (and only) Sharded gossiping strategy

#![warn(missing_docs)]

use crate::agent_store::AgentInfoSigned;
use crate::gossip::{decode_bloom_filter, encode_bloom_filter};
use crate::types::event::*;
use crate::types::gossip::*;
use crate::{types::*, HostApi};
use ghost_actor::dependencies::tracing;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::RateLimiter;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::codec::Codec;
use kitsune_p2p_types::combinators::second;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::dht::region_set::RegionSetLtcs;
use kitsune_p2p_types::dht_arc::{DhtArcRange, DhtArcSet};
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::{TryFrom, TryInto};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time::Instant;

pub use self::bandwidth::BandwidthThrottle;
use self::ops::OpsBatchQueue;
use self::state_map::RoundStateMap;
use crate::metrics::MetricsSync;

use super::{HowToConnect, MetaOpKey};

pub use bandwidth::BandwidthThrottles;

#[cfg(any(test, feature = "test_utils"))]
#[allow(missing_docs)]
pub mod test_utils;

mod accept;
mod agents;
mod bloom;
mod initiate;
mod ops;
mod state_map;
mod store;

mod bandwidth;
mod next_target;

// dead_code and unused_imports are allowed here because when compiling this
// code path due to test_utils, the helper functions defined in this module
// are not used due to the tests themselves not being compiled, so it's easier
// to do this than to annotate each function as `#[cfg(test)]`
#[cfg(any(test, feature = "test_utils"))]
#[allow(dead_code)]
#[allow(unused_imports)]
pub(crate) mod tests;

/// max send buffer size (keep it under 16384 with a little room for overhead)
/// (this is not a tuning_param because it must be coordinated
/// with the constant in PoolBuf which cannot be set at runtime)
/// ^^ obviously we're no longer following the above advice..
///    in the case of the pool buf management, any gossips larger than
///    16,000,000 will now be shrunk resulting in additional memory thrashing
const MAX_SEND_BUF_BYTES: usize = 16_000_000;

/// The timeout for a gossip round if there is no contact. One minute.
const ROUND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

type BloomFilter = bloomfilter::Bloom<MetaOpKey>;
type EventSender = futures::channel::mpsc::Sender<event::KitsuneP2pEvent>;

#[derive(Debug)]
struct TimedBloomFilter {
    /// The bloom filter for the time window.
    /// If this is none then we have no hashes
    /// for this time window.
    bloom: Option<BloomFilter>,
    /// The time window for this bloom filter.
    time: TimeWindow,
}

/// Gossip has two distinct variants which share a lot of similarities but
/// are fundamentally different and serve different purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GossipType {
    /// The Recent gossip type is aimed at rapidly syncing the most recent
    /// data. It runs frequently and expects frequent diffs at each round.
    Recent,
    /// The Historical gossip type is aimed at comprehensively syncing the
    /// entire common history of two nodes, filling in gaps in the historical
    /// data. It runs less frequently, and expects diffs to be infrequent
    /// at each round.
    Historical,
}

/// The entry point for the sharded gossip strategy.
///
/// This struct encapsulates the network communication concerns, mainly
/// managing the incoming and outgoing gossip queues. It contains a struct
/// which handles all other (local) aspects of gossip.
pub struct ShardedGossip {
    /// ShardedGossipLocal handles the non-networking concerns of gossip
    gossip: ShardedGossipLocal,
    // The endpoint to use for all outgoing comms
    ep_hnd: Tx2EpHnd<wire::Wire>,
    /// The internal mutable state
    pub(crate) state: Share<ShardedGossipState>,
    /// Bandwidth for incoming and outgoing gossip.
    bandwidth: Arc<BandwidthThrottle>,
}

impl std::fmt::Debug for ShardedGossip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShardedGossip{...}").finish()
    }
}

/// Basic statistic for gossip loop processing performance.
struct Stats {
    start: Instant,
    last: Option<tokio::time::Instant>,
    avg_processing_time: std::time::Duration,
    max_processing_time: std::time::Duration,
    count: u32,
}

impl std::fmt::Display for GossipType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GossipType::Recent => write!(f, "recent"),
            GossipType::Historical => write!(f, "historical"),
        }
    }
}

impl Stats {
    /// Reset the stats.
    fn reset() -> Self {
        Stats {
            start: Instant::now(),
            last: None,
            avg_processing_time: std::time::Duration::default(),
            max_processing_time: std::time::Duration::default(),
            count: 0,
        }
    }
}

impl ShardedGossip {
    /// Constructor
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: EventSender,
        host_api: HostApi,
        gossip_type: GossipType,
        bandwidth: Arc<BandwidthThrottle>,
        metrics: MetricsSync,
        enable_history: bool,
    ) -> Arc<Self> {
        let state = if enable_history {
            ShardedGossipState::with_history()
        } else {
            Default::default()
        };
        let this = Arc::new(Self {
            ep_hnd,
            state: Share::new(state),
            gossip: ShardedGossipLocal {
                tuning_params,
                space,
                evt_sender,
                host_api,
                inner: Share::new(ShardedGossipLocalState::new(metrics)),
                gossip_type,
                closing: AtomicBool::new(false),
            },
            bandwidth,
        });
        metric_task({
            let this = this.clone();

            async move {
                let mut stats = Stats::reset();
                while !this
                    .gossip
                    .closing
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    this.run_one_iteration().await;
                    this.stats(&mut stats);
                }
                KitsuneResult::Ok(())
            }
        });
        this
    }

    async fn process_outgoing(&self, outgoing: Outgoing) -> KitsuneResult<()> {
        let (cert, how, gossip) = outgoing;
        match self.gossip.gossip_type {
            GossipType::Recent => {
                let s = tracing::trace_span!("process_outgoing_recent", cert = ?cert, agents = ?self.gossip.show_local_agents());
                s.in_scope(|| tracing::trace!(?gossip));
            }
            GossipType::Historical => {
                let s = tracing::trace_span!("process_outgoing_historical", cert = ?cert, agents = ?self.gossip.show_local_agents());
                match &gossip {
                    ShardedGossipWire::MissingOps(MissingOps { ops, finished }) => {
                        s.in_scope(|| tracing::trace!(num_ops = %ops.len(), ?finished));
                    }
                    _ => {
                        s.in_scope(|| tracing::trace!(?gossip));
                    }
                }
            }
        };
        let gossip = gossip.encode_vec().map_err(KitsuneError::other)?;
        let bytes = gossip.len();
        let gossip = wire::Wire::gossip(
            self.gossip.space.clone(),
            gossip.into(),
            self.gossip.gossip_type.into(),
        );

        let timeout = self.gossip.tuning_params.implicit_timeout();

        self.bandwidth.outgoing_bytes(bytes).await;

        let con = match how.clone() {
            HowToConnect::Con(con, remote_url) => {
                if con.is_closed() {
                    self.ep_hnd.get_connection(remote_url, timeout).await?
                } else {
                    con
                }
            }
            HowToConnect::Url(url) => self.ep_hnd.get_connection(url, timeout).await?,
        };
        // Wait for enough available outgoing bandwidth here before
        // actually sending the gossip.
        con.notify(&gossip, timeout).await?;
        Ok(())
    }

    async fn process_incoming_outgoing(&self) -> KitsuneResult<()> {
        let (incoming, outgoing) = self.pop_queues()?;
        if let Some((con, remote_url, msg, bytes)) = incoming {
            self.bandwidth.incoming_bytes(bytes).await;
            let outgoing = match self.gossip.process_incoming(con.peer_cert(), msg).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("FAILED to process incoming gossip {:?}", e);
                    self.gossip.remove_state(&con.peer_cert(), true)?;
                    vec![ShardedGossipWire::error(e.to_string())]
                }
            };
            self.state.share_mut(|i, _| {
                i.push_outgoing(outgoing.into_iter().map(|msg| {
                    (
                        con.peer_cert(),
                        HowToConnect::Con(con.clone(), remote_url.clone()),
                        msg,
                    )
                }));
                Ok(())
            })?;
        }
        if let Some(outgoing) = outgoing {
            let cert = outgoing.0.clone();
            if let Err(err) = self.process_outgoing(outgoing).await {
                self.gossip.remove_state(&cert, true)?;
                tracing::error!(
                    "Gossip failed to send outgoing message because of: {:?}",
                    err
                );
            }
        }

        Ok(())
    }

    async fn run_one_iteration(&self) {
        match self.gossip.try_initiate().await {
            Ok(Some(outgoing)) => {
                if let Err(err) = self.state.share_mut(|i, _| {
                    i.push_outgoing([outgoing]);
                    Ok(())
                }) {
                    tracing::error!(
                        "Gossip failed to get share nut when trying to initiate with {:?}",
                        err
                    );
                }
            }
            Ok(None) => (),
            Err(err) => tracing::error!("Gossip failed when trying to initiate with {:?}", err),
        }
        if let Err(err) = self.process_incoming_outgoing().await {
            tracing::error!("Gossip failed to process a message because of: {:?}", err);
        }
        self.gossip.record_timeouts();
    }

    fn pop_queues(&self) -> KitsuneResult<(Option<Incoming>, Option<Outgoing>)> {
        self.state.share_mut(move |inner, _| Ok(inner.pop()))
    }

    /// Log the statistics for the gossip loop.
    fn stats(&self, stats: &mut Stats) {
        if let Some(last) = stats.last {
            let elapsed = last.elapsed();
            stats.avg_processing_time += elapsed;
            stats.max_processing_time = std::cmp::max(stats.max_processing_time, elapsed);
        }
        stats.last = Some(tokio::time::Instant::now());
        stats.count += 1;
        let elapsed = stats.start.elapsed();
        if elapsed.as_secs() > 5 {
            stats.avg_processing_time = stats
                .avg_processing_time
                .checked_div(stats.count)
                .unwrap_or_default();
            let lens = self
                .state
                .share_mut(|i, _| Ok((i.incoming.len(), i.outgoing.len())))
                .map(|(i, o)| format!("Queues: Incoming: {}, Outgoing {}", i, o))
                .unwrap_or_else(|_| "Queues empty".to_string());
            let _ = self.gossip.inner.share_mut(|i, _| {
                    let s = tracing::trace_span!("gossip_metrics", gossip_type = %self.gossip.gossip_type);
                    s.in_scope(|| tracing::trace!("{}\nStats over last 5s:\n\tAverage processing time {:?}\n\tIteration count: {}\n\tMax gossip processing time: {:?}\n\t{}", i.metrics, stats.avg_processing_time, stats.count, stats.max_processing_time, lens));
                    Ok(())
                });
            *stats = Stats::reset();
        }
    }
}

/// The parts of sharded gossip which are concerned only with the gossiping node:
/// - managing local state
/// - making requests to the local backend
/// - processing incoming messages to produce outgoing messages (which actually)
///     get sent by the enclosing `ShardedGossip`
pub struct ShardedGossipLocal {
    gossip_type: GossipType,
    tuning_params: KitsuneP2pTuningParams,
    space: Arc<KitsuneSpace>,
    evt_sender: EventSender,
    host_api: HostApi,
    inner: Share<ShardedGossipLocalState>,
    closing: AtomicBool,
}

/// Incoming gossip.
type Incoming = (Tx2ConHnd<wire::Wire>, TxUrl, ShardedGossipWire, usize);
/// Outgoing gossip.
type Outgoing = (Tx2Cert, HowToConnect, ShardedGossipWire);

type StateKey = Tx2Cert;

/// Info associated with an outgoing gossip target
#[derive(Debug)]
pub(crate) struct ShardedGossipTarget {
    pub(crate) remote_agent_list: Vec<AgentInfoSigned>,
    pub(crate) cert: Tx2Cert,
    pub(crate) tie_break: u32,
    pub(crate) when_initiated: Option<tokio::time::Instant>,
    #[allow(dead_code)]
    pub(crate) url: TxUrl,
}

/// The internal mutable state for [`ShardedGossipLocal`]
#[derive(Default)]
pub struct ShardedGossipLocalState {
    /// The list of agents on this node
    local_agents: HashSet<Arc<KitsuneAgent>>,
    /// If Some, we are in the process of trying to initiate gossip with this target.
    initiate_tgt: Option<ShardedGossipTarget>,
    round_map: RoundStateMap,
    /// Metrics that track remote node states and help guide
    /// the next node to gossip with.
    metrics: MetricsSync,
}

impl ShardedGossipLocalState {
    fn new(metrics: MetricsSync) -> Self {
        Self {
            metrics,
            ..Default::default()
        }
    }

    fn remove_state(&mut self, state_key: &StateKey, error: bool) -> Option<RoundState> {
        // Check if the round to be removed matches the current initiate_tgt
        let init_tgt = self
            .initiate_tgt
            .as_ref()
            .map(|tgt| &tgt.cert == state_key)
            .unwrap_or(false);
        let remote_agent_list = if init_tgt {
            let initiate_tgt = self.initiate_tgt.take().unwrap();
            initiate_tgt.remote_agent_list
        } else {
            vec![]
        };
        let r = self.round_map.remove(state_key);
        if let Some(r) = &r {
            if error {
                self.metrics.write().record_error(&r.remote_agent_list);
            } else {
                self.metrics.write().record_success(&r.remote_agent_list);
            }
        } else if init_tgt && error {
            self.metrics.write().record_error(&remote_agent_list);
        }
        r
    }

    fn check_tgt_expired(&mut self) {
        if let Some((remote_agent_list, cert, when_initiated)) = self
            .initiate_tgt
            .as_ref()
            .map(|tgt| (&tgt.remote_agent_list, tgt.cert.clone(), tgt.when_initiated))
        {
            // Check if no current round exists and we've timed out the initiate.
            let no_current_round_exist = !self.round_map.round_exists(&cert);
            match when_initiated {
                Some(when_initiated)
                    if no_current_round_exist && when_initiated.elapsed() > ROUND_TIMEOUT =>
                {
                    tracing::error!("Tgt expired {:?}", cert);
                    self.metrics.write().record_error(remote_agent_list);
                    self.initiate_tgt = None;
                }
                None if no_current_round_exist => {
                    self.initiate_tgt = None;
                }
                _ => (),
            }
        }
    }

    fn new_integrated_data(&mut self) -> KitsuneResult<()> {
        let s = tracing::trace_span!("gossip_trigger", agents = ?self.show_local_agents());
        s.in_scope(|| self.log_state());
        self.metrics.write().record_force_initiate();
        Ok(())
    }

    fn show_local_agents(&self) -> &HashSet<Arc<KitsuneAgent>> {
        &self.local_agents
    }

    fn log_state(&self) {
        tracing::trace!(
            ?self.round_map,
            ?self.initiate_tgt,
        )
    }
}

/// The incoming and outgoing queues for [`ShardedGossip`]
#[derive(Default, Clone, Debug)]
pub struct ShardedGossipQueues {
    incoming: VecDeque<Incoming>,
    outgoing: VecDeque<Outgoing>,
}

/// The internal mutable state for [`ShardedGossip`]
#[derive(Default, derive_more::Deref)]
pub(crate) struct ShardedGossipState {
    /// The incoming and outgoing queues
    #[deref]
    queues: ShardedGossipQueues,
    /// If Some, these queues are never cleared, and contain every message
    /// ever sent and received, for diagnostics and debugging.
    history: Option<ShardedGossipQueues>,
}

impl ShardedGossipState {
    /// Construct state with history queues
    pub fn with_history() -> Self {
        Self {
            queues: Default::default(),
            history: Some(Default::default()),
        }
    }

    #[cfg(feature = "test_utils")]
    #[allow(dead_code)]
    pub fn get_history(&self) -> Option<ShardedGossipQueues> {
        self.history.clone()
    }

    pub fn push_incoming<I: Clone + IntoIterator<Item = Incoming>>(&mut self, incoming: I) {
        if let Some(history) = &mut self.history {
            history.incoming.extend(incoming.clone().into_iter());
        }
        self.queues.incoming.extend(incoming.into_iter());
    }

    pub fn push_outgoing<I: Clone + IntoIterator<Item = Outgoing>>(&mut self, outgoing: I) {
        if let Some(history) = &mut self.history {
            history.outgoing.extend(outgoing.clone().into_iter());
        }
        self.queues.outgoing.extend(outgoing.into_iter());
    }

    pub fn pop(&mut self) -> (Option<Incoming>, Option<Outgoing>) {
        (
            self.queues.incoming.pop_front(),
            self.queues.outgoing.pop_front(),
        )
    }
}

/// The state representing a single active ongoing "round" of gossip with a
/// remote node
#[derive(Debug, Clone)]
pub struct RoundState {
    /// The remote agents hosted by the remote node, used for metrics tracking
    remote_agent_list: Vec<AgentInfoSigned>,
    /// The common ground with our gossip partner for the purposes of this round
    common_arc_set: Arc<DhtArcSet>,
    /// Number of ops blooms we have sent for this round, which is also the
    /// number of MissingOps sets we expect in response
    num_sent_op_blooms: u8,
    /// We've received the last op bloom filter from our partner
    /// (the one with `finished` == true)
    received_all_incoming_op_blooms: bool,
    /// Received all responses to OpRegions, which is the batched set of Op data
    /// in the diff of regions
    has_pending_historical_op_data: bool,
    /// There are still op blooms to send because the previous
    /// batch was too big to send in a single gossip iteration.
    bloom_batch_cursor: Option<Timestamp>,
    /// Missing op hashes that have been batched for
    /// future processing.
    ops_batch_queue: OpsBatchQueue,
    /// Last moment we had any contact for this round.
    last_touch: Instant,
    /// Amount of time before a round is considered expired.
    round_timeout: std::time::Duration,
    /// The RegionSet we will send to our gossip partner during Historical
    /// gossip (will be None for Recent).
    region_set_sent: Option<Arc<RegionSetLtcs>>,
}

impl ShardedGossipLocal {
    const TGT_FP: f64 = 0.01;
    /// This should give us just under 1.6MB for the bloom filter.
    /// Based on a compression of 75%.
    const UPPER_HASHES_BOUND: usize = 20_000;

    /// The number of bloom filters we want to send in a single gossip iteration.
    const UPPER_BLOOM_BOUND: usize = 10;

    /// Calculate the time range for a gossip round.
    fn calculate_time_range(&self) -> TimeWindow {
        const NOW: Duration = Duration::from_secs(0);
        let threshold = Duration::from_secs(self.tuning_params.danger_gossip_recent_threshold_secs);
        match self.gossip_type {
            GossipType::Recent => time_range(threshold, NOW),
            GossipType::Historical => {
                let one_hour_ago = std::time::UNIX_EPOCH
                    .elapsed()
                    .expect("Your clock is set before unix epoch")
                    - threshold;
                Timestamp::from_micros(0)
                    ..Timestamp::from_micros(
                        one_hour_ago
                            .as_micros()
                            .try_into()
                            .expect("Epoch micro seconds has overflowed"),
                    )
            }
        }
    }

    fn new_state(
        &self,
        remote_agent_list: Vec<AgentInfoSigned>,
        common_arc_set: Arc<DhtArcSet>,
        region_set_sent: Option<RegionSetLtcs>,
    ) -> KitsuneResult<RoundState> {
        Ok(RoundState {
            remote_agent_list,
            common_arc_set,
            num_sent_op_blooms: 0,
            received_all_incoming_op_blooms: false,
            has_pending_historical_op_data: false,
            bloom_batch_cursor: None,
            ops_batch_queue: OpsBatchQueue::new(),
            last_touch: Instant::now(),
            round_timeout: ROUND_TIMEOUT,
            region_set_sent: region_set_sent.map(Arc::new),
        })
    }

    fn get_state(&self, id: &StateKey) -> KitsuneResult<Option<RoundState>> {
        self.inner
            .share_mut(|i, _| Ok(i.round_map.get(id).cloned()))
    }

    fn remove_state(&self, id: &StateKey, error: bool) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| Ok(i.remove_state(id, error)))
    }

    fn remove_target(&self, id: &StateKey, error: bool) -> KitsuneResult<()> {
        self.inner.share_mut(|i, _| {
            if i.initiate_tgt
                .as_ref()
                .map(|tgt| &tgt.cert == id)
                .unwrap_or(false)
            {
                let initiate_tgt = i.initiate_tgt.take().unwrap();
                if error {
                    i.metrics
                        .write()
                        .record_error(&initiate_tgt.remote_agent_list);
                } else {
                    i.metrics
                        .write()
                        .record_success(&initiate_tgt.remote_agent_list);
                }
            }
            Ok(())
        })
    }

    /// If the round is still active then update the state.
    fn update_state_if_active(&self, key: StateKey, state: RoundState) -> KitsuneResult<()> {
        self.inner.share_mut(|i, _| {
            if i.round_map.round_exists(&key) {
                if state.is_finished() {
                    i.remove_state(&key, false);
                } else {
                    i.round_map.insert(key, state);
                }
            }
            Ok(())
        })
    }

    fn incoming_op_blooms_finished(
        &self,
        state_id: &StateKey,
    ) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| {
            let finished = i
                .round_map
                .get_mut(state_id)
                .map(|state| {
                    state.received_all_incoming_op_blooms = true;
                    state.is_finished()
                })
                .unwrap_or(true);
            if finished {
                Ok(i.remove_state(state_id, false))
            } else {
                Ok(i.round_map.get(state_id).cloned())
            }
        })
    }

    fn decrement_op_blooms(&self, state_id: &StateKey) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| {
            let update_state = |state: &mut RoundState| {
                let num_op_blooms = state.num_sent_op_blooms.saturating_sub(1);
                state.num_sent_op_blooms = num_op_blooms;
                // NOTE: there is only ever one "batch" of OpRegions
                state.has_pending_historical_op_data = false;
                state.is_finished()
            };
            if i.round_map
                .get_mut(state_id)
                .map(update_state)
                .unwrap_or(true)
            {
                Ok(i.remove_state(state_id, false))
            } else {
                Ok(i.round_map.get(state_id).cloned())
            }
        })
    }

    async fn process_incoming(
        &self,
        cert: Tx2Cert,
        msg: ShardedGossipWire,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        let s = match self.gossip_type {
            GossipType::Recent => {
                let s = tracing::trace_span!("process_incoming_recent", ?cert, agents = ?self.show_local_agents(), ?msg);
                s.in_scope(|| self.log_state());
                s
            }
            GossipType::Historical => match &msg {
                ShardedGossipWire::MissingOps(MissingOps { ops, finished }) => {
                    let s = tracing::trace_span!("process_incoming_historical", ?cert, agents = ?self.show_local_agents(), msg = %"MissingOps", num_ops = %ops.len(), ?finished);
                    s.in_scope(|| self.log_state());
                    s
                }
                _ => {
                    let s = tracing::trace_span!("process_incoming_historical", ?cert, agents = ?self.show_local_agents(), ?msg);
                    s.in_scope(|| self.log_state());
                    s
                }
            },
        };
        // If we don't have the state for a message then the other node will need to timeout.
        let r = match msg {
            ShardedGossipWire::Initiate(Initiate {
                intervals,
                id,
                agent_list,
            }) => {
                self.incoming_initiate(cert, intervals, id, agent_list)
                    .await?
            }
            ShardedGossipWire::Accept(Accept {
                intervals,
                agent_list,
            }) => self.incoming_accept(cert, intervals, agent_list).await?,
            ShardedGossipWire::Agents(Agents { filter }) => {
                if let Some(state) = self.get_state(&cert)? {
                    let filter = decode_bloom_filter(&filter);
                    self.incoming_agents(state, filter).await?
                } else {
                    Vec::with_capacity(0)
                }
            }
            ShardedGossipWire::MissingAgents(MissingAgents { agents }) => {
                if self.get_state(&cert)?.is_some() {
                    self.incoming_missing_agents(agents.as_slice()).await?;
                }
                Vec::with_capacity(0)
            }
            ShardedGossipWire::OpBloom(OpBloom {
                missing_hashes,
                finished,
            }) => {
                let state = if finished {
                    self.incoming_op_blooms_finished(&cert)?
                } else {
                    self.get_state(&cert)?
                };
                match state {
                    Some(state) => match missing_hashes {
                        EncodedTimedBloomFilter::NoOverlap => Vec::with_capacity(0),
                        EncodedTimedBloomFilter::MissingAllHashes { time_window } => {
                            let filter = TimedBloomFilter {
                                bloom: None,
                                time: time_window,
                            };
                            self.incoming_op_bloom(state, filter, None).await?
                        }
                        EncodedTimedBloomFilter::HaveHashes {
                            filter,
                            time_window,
                        } => {
                            let filter = TimedBloomFilter {
                                bloom: Some(decode_bloom_filter(&filter)),
                                time: time_window,
                            };
                            self.incoming_op_bloom(state, filter, None).await?
                        }
                    },
                    None => Vec::with_capacity(0),
                }
            }
            ShardedGossipWire::OpRegions(OpRegions { region_set }) => {
                if let Some(state) = self.incoming_op_blooms_finished(&cert)? {
                    self.queue_incoming_regions(state, region_set).await?
                } else {
                    vec![]
                }
            }
            ShardedGossipWire::MissingOps(MissingOps { ops, finished }) => {
                let mut gossip = Vec::with_capacity(0);
                let finished = MissingOpsStatus::try_from(finished)?;

                let state = match finished {
                    // This is a single chunk of ops. No need to reply.
                    MissingOpsStatus::ChunkComplete => self.get_state(&cert)?,
                    // This is the last chunk in the batch. Reply with [`OpBatchReceived`]
                    // to get the next batch of missing ops.
                    MissingOpsStatus::BatchComplete => {
                        gossip = vec![ShardedGossipWire::op_batch_received()];
                        self.get_state(&cert)?
                    }
                    // All the batches of missing ops for the bloom this node sent
                    // to the remote node have been sent back to this node.
                    MissingOpsStatus::AllComplete => {
                        // This node can decrement the number of outstanding ops bloom replies
                        // it is waiting for.
                        let mut state = self.decrement_op_blooms(&cert)?;

                        // If there are more blooms to send because this node had to batch the blooms
                        // and all the outstanding blooms have been received then this node will send
                        // the next batch of ops blooms starting from the saved cursor.
                        if let Some(state) = state
                            .as_mut()
                            .filter(|s| s.bloom_batch_cursor.is_some() && s.num_sent_op_blooms == 0)
                        {
                            // We will be producing some gossip so we need to allocate.
                            gossip = Vec::new();
                            // Generate the next ops blooms batch.
                            *state = self.next_bloom_batch(state.clone(), &mut gossip).await?;
                            // Update the state.
                            self.update_state_if_active(cert.clone(), state.clone())?;
                        }
                        state
                    }
                };

                // TODO: come back to this later after implementing batching for
                //      region gossip, for now I just don't care about the state,
                //      and just want to handle the incoming ops.
                if (self.gossip_type == GossipType::Historical || state.is_some())
                    && !ops.is_empty()
                {
                    self.incoming_missing_ops(ops).await?;
                }
                gossip
            }
            ShardedGossipWire::OpBatchReceived(_) => match self.get_state(&cert)? {
                Some(state) => {
                    // The last ops batch has been received by the
                    // remote node so now send the next batch.
                    let r = self.next_missing_ops_batch(state.clone()).await?;
                    if state.is_finished() {
                        self.remove_state(&cert, false)?;
                    }
                    r
                }
                None => Vec::with_capacity(0),
            },
            ShardedGossipWire::NoAgents(_) => {
                tracing::warn!("No agents to gossip with on the node {:?}", cert);
                self.remove_state(&cert, true)?;
                Vec::with_capacity(0)
            }
            ShardedGossipWire::AlreadyInProgress(_) => {
                self.remove_target(&cert, false)?;
                Vec::with_capacity(0)
            }
            ShardedGossipWire::Busy(_) => {
                tracing::warn!("The node {:?} is busy", cert);
                self.remove_target(&cert, true)?;
                Vec::with_capacity(0)
            }
            ShardedGossipWire::Error(Error { message }) => {
                tracing::warn!("gossiping with: {:?} and got error: {}", cert, message);
                self.remove_state(&cert, true)?;
                Vec::with_capacity(0)
            }
        };
        s.in_scope(|| {
            let ops_s = r
                .iter()
                .map(|g| match &g {
                    ShardedGossipWire::MissingOps(MissingOps { ops, finished }) => {
                        format!("num_ops = {}, finished = {}", ops.len(), finished)
                    }
                    _ => {
                        format!("{:?}", g)
                    }
                })
                .collect::<String>();
            tracing::trace!(%ops_s);
            self.log_state()
        });
        Ok(r)
    }

    /// Record all timed out rounds into metrics
    fn record_timeouts(&self) {
        self.inner
            .share_mut(|i, _| {
                for (cert, r) in i.round_map.take_timed_out_rounds() {
                    tracing::warn!("The node {:?} has timed out their gossip round", cert);
                    i.metrics.write().record_error(&r.remote_agent_list);
                }
                Ok(())
            })
            .ok();
    }

    fn show_local_agents(&self) -> HashSet<Arc<KitsuneAgent>> {
        self.inner
            .share_mut(|i, _| Ok(i.local_agents.clone()))
            .unwrap_or_default()
    }

    fn log_state(&self) {
        self.inner
            .share_mut(|i, _| {
                i.log_state();
                Ok(())
            })
            .ok();
    }
}

impl RoundState {
    fn increment_sent_op_blooms(&mut self) -> u8 {
        self.num_sent_op_blooms += 1;
        self.num_sent_op_blooms
    }

    /// A round is finished if:
    /// - There are no blooms sent to the remote node that are awaiting responses.
    /// - This node has received all the ops blooms from the remote node.
    /// - This node has no saved ops bloom batch cursor.
    /// - This node has no queued missing ops to send to the remote node.
    fn is_finished(&self) -> bool {
        self.num_sent_op_blooms == 0
            && !self.has_pending_historical_op_data
            && self.received_all_incoming_op_blooms
            && self.bloom_batch_cursor.is_none()
            && self.ops_batch_queue.is_empty()
    }
}

/// Time range from now into the past.
/// Start must be < end.
fn time_range(start: Duration, end: Duration) -> TimeWindow {
    // TODO: write in terms of chrono::now()
    let now = SystemTime::now();
    let start = now
        .checked_sub(start)
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|t| Timestamp::from_micros(t.as_micros() as i64))
        .unwrap_or(Timestamp::MIN);

    let end = now
        .checked_sub(end)
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|t| Timestamp::from_micros(t.as_micros() as i64))
        .unwrap_or(Timestamp::MAX);

    start..end
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
/// An encoded timed bloom filter of missing op hashes.
pub enum EncodedTimedBloomFilter {
    /// I have no overlap with your agents
    /// Pleas don't send any ops.
    NoOverlap,
    /// I have overlap and I have no hashes.
    /// Please send all your ops.
    MissingAllHashes {
        /// The time window that we are missing hashes for.
        time_window: TimeWindow,
    },
    /// I have overlap and I have some hashes.
    /// Please send any missing ops.
    HaveHashes {
        /// The encoded bloom filter.
        filter: PoolBuf,
        /// The time window these hashes are for.
        time_window: TimeWindow,
    },
}

#[derive(Debug, Clone, Copy)]
/// The possible states when receiving missing ops.
/// Note this is not sent over the wire and is instead
/// converted to a u8 to save bandwidth.
pub enum MissingOpsStatus {
    /// There are more chunks in this batch to come. No reply is needed.
    ChunkComplete = 0,
    /// This chunk is done but there are more batches
    /// to come and you should reply with [`OpBatchReceived`]
    /// when you are ready to get the next batch.
    BatchComplete = 1,
    /// This is the final batch of missing ops and there
    /// are no more ops to come. No reply is needed.
    AllComplete = 2,
}

kitsune_p2p_types::write_codec_enum! {
    /// SimpleBloom Gossip Wire Protocol Codec
    codec ShardedGossipWire {
        /// Initiate a round of gossip with a remote node
        Initiate(0x10) {
            /// The list of arc intervals (equivalent to a [`DhtArcSet`])
            /// for all local agents
            intervals.0: Vec<DhtArcRange>,
            /// A random number to resolve concurrent initiates.
            id.1: u32,
            /// List of active local agents represented by this node.
            agent_list.2: Vec<AgentInfoSigned>,
        },

        /// Accept an incoming round of gossip from a remote node
        Accept(0x20) {
            /// The list of arc intervals (equivalent to a [`DhtArcSet`])
            /// for all local agents
            intervals.0: Vec<DhtArcRange>,
            /// List of active local agents represented by this node.
            agent_list.1: Vec<AgentInfoSigned>,
        },

        /// Send Agent Info Bloom
        Agents(0x30) {
            /// The bloom filter for agent data
            filter.0: PoolBuf,
        },

        /// Any agents that were missing from the remote bloom.
        MissingAgents(0x40) {
            /// The missing agents
            agents.0: Vec<Arc<AgentInfoSigned>>,
        },

        /// Send Op Bloom filter
        OpBloom(0x50) {
            /// The bloom filter for op data
            missing_hashes.0: EncodedTimedBloomFilter,
            /// Is this the last bloom to be sent?
            finished.1: bool,
        },

        /// Send Op region hashes
        OpRegions(0x51) {
            /// The region hashes for all common ops
            region_set.0: RegionSetLtcs,
        },

        /// Any ops that were missing from the remote bloom.
        MissingOps(0x60) {
            /// The missing ops
            ops.0: Vec<KOp>,
            /// Ops that are missing from a bloom that you have sent.
            /// These will be chunked into a maximum size of about 16MB.
            /// If the amount of missing ops is larger then the
            /// [`ShardedGossipLocal::UPPER_BATCH_BOUND`] then the set of
            /// missing ops chunks will be sent in batches.
            /// Each batch will require a reply message of [`OpBatchReceived`]
            /// in order to get the next batch.
            /// This is to prevent overloading the receiver with too much
            /// incoming data.
            ///
            /// 0: There is more chunks in this batch to come. No reply is needed.
            /// 1: This chunk is done but there is more batches
            /// to come and you should reply with [`OpBatchReceived`]
            /// when you are ready to get the next batch.
            /// 2: This is the final missing ops and there
            /// are no more ops to come. No reply is needed.
            ///
            /// See [`MissingOpsStatus`]
            finished.1: u8,
        },

        /// I have received a complete batch of
        /// missing ops and I am ready to receive the
        /// next batch.
        OpBatchReceived(0x61) {
        },


        /// The node you are gossiping with has hit an error condition
        /// and failed to respond to a request.
        Error(0xa0) {
            /// The error message.
            message.0: String,
        },

        /// The node currently is gossiping with too many
        /// other nodes and is too busy to accept your initiate.
        /// Please try again later.
        Busy(0xa1) {
        },

        /// The node you are trying to gossip with has no agents anymore.
        NoAgents(0xa2) {
        },

        /// You have sent a stale initiate to a node
        /// that already has an active round with you.
        AlreadyInProgress(0xa3) {
        },
    }
}

impl AsGossipModule for ShardedGossip {
    fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        remote_url: TxUrl,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()> {
        use kitsune_p2p_types::codec::*;
        let (bytes, gossip) =
            ShardedGossipWire::decode_ref(&gossip_data).map_err(KitsuneError::other)?;
        let new_initiate = matches!(gossip, ShardedGossipWire::Initiate(_));
        self.state.share_mut(move |i, _| {
            let overloaded = i.incoming.len() > 20;
            if overloaded {
                tracing::warn!(
                    "Overloaded with incoming gossip.. {} messages",
                    i.incoming.len()
                );
            }
            // If we are overloaded then return busy to any new initiates.
            if overloaded && new_initiate {
                i.push_outgoing([(
                    con.peer_cert(),
                    HowToConnect::Con(con, remote_url),
                    ShardedGossipWire::busy(),
                )]);
            } else {
                i.push_incoming([(con, remote_url, gossip, bytes as usize)]);
            }
            Ok(())
        })
    }

    fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        let _ = self.gossip.inner.share_mut(move |i, _| {
            i.new_integrated_data()?;
            i.local_agents.insert(a);
            let s = tracing::trace_span!("gossip_trigger", agents = ?i.show_local_agents(), msg = "New agent joining");
            s.in_scope(|| i.log_state());
            Ok(())
        });
    }

    fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        let _ = self.gossip.inner.share_mut(move |i, _| {
            i.local_agents.remove(&a);
            Ok(())
        });
    }

    fn close(&self) {
        self.gossip
            .closing
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn new_integrated_data(&self) {
        let _ = self.gossip.inner.share_mut(move |i, _| {
            i.new_integrated_data()?;
            let s = tracing::trace_span!("gossip_trigger", agents = ?i.show_local_agents(), msg = "New integrated data");
            s.in_scope(|| i.log_state());
            Ok(())
        });
    }
}

struct ShardedRecentGossipFactory {
    bandwidth: Arc<BandwidthThrottle>,
}

impl ShardedRecentGossipFactory {
    fn new(bandwidth: Arc<BandwidthThrottle>) -> Self {
        Self { bandwidth }
    }
}

impl AsGossipModuleFactory for ShardedRecentGossipFactory {
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
        host: HostApi,
        metrics: MetricsSync,
    ) -> GossipModule {
        GossipModule(ShardedGossip::new(
            tuning_params,
            space,
            ep_hnd,
            evt_sender,
            host,
            GossipType::Recent,
            self.bandwidth.clone(),
            metrics,
            false,
        ))
    }
}

struct ShardedHistoricalGossipFactory {
    bandwidth: Arc<BandwidthThrottle>,
}

impl ShardedHistoricalGossipFactory {
    fn new(bandwidth: Arc<BandwidthThrottle>) -> Self {
        Self { bandwidth }
    }
}

impl AsGossipModuleFactory for ShardedHistoricalGossipFactory {
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
        host: HostApi,
        metrics: MetricsSync,
    ) -> GossipModule {
        GossipModule(ShardedGossip::new(
            tuning_params,
            space,
            ep_hnd,
            evt_sender,
            host,
            GossipType::Historical,
            self.bandwidth.clone(),
            metrics,
            false,
        ))
    }
}

/// Create a recent `GossipModuleFactory`
pub fn recent_factory(bandwidth: Arc<BandwidthThrottle>) -> GossipModuleFactory {
    GossipModuleFactory(Arc::new(ShardedRecentGossipFactory::new(bandwidth)))
}

/// Create a historical `GossipModuleFactory`
pub fn historical_factory(bandwidth: Arc<BandwidthThrottle>) -> GossipModuleFactory {
    GossipModuleFactory(Arc::new(ShardedHistoricalGossipFactory::new(bandwidth)))
}

#[allow(dead_code)]
fn clamp64(u: u64) -> i64 {
    if u > i64::MAX as u64 {
        i64::MAX
    } else {
        u as i64
    }
}

impl From<GossipType> for GossipModuleType {
    fn from(g: GossipType) -> Self {
        match g {
            GossipType::Recent => GossipModuleType::ShardedRecent,
            GossipType::Historical => GossipModuleType::ShardedHistorical,
        }
    }
}

impl TryFrom<u8> for MissingOpsStatus {
    type Error = KitsuneError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let r = match value {
            0 => Self::ChunkComplete,
            1 => Self::BatchComplete,
            2 => Self::AllComplete,
            _ => return Err("Failed to parse u8 as MissingOpsStatus".into()),
        };
        debug_assert_eq!(value, r as u8);
        Ok(r)
    }
}
