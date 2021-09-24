//! The main (and only) Sharded gossiping strategy

#![warn(missing_docs)]

use crate::agent_store::AgentInfoSigned;
use crate::gossip::simple_bloom::{decode_bloom_filter, encode_bloom_filter};
use crate::types::event::*;
use crate::types::gossip::*;
use crate::types::*;
use ghost_actor::dependencies::tracing;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::RateLimiter;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::codec::Codec;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::dht_arc::{ArcInterval, DhtArcSet};
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use self::bandwidth::BandwidthThrottle;
use self::metrics::Metrics;
use self::state_map::RoundStateMap;

use super::simple_bloom::{HowToConnect, MetaOpKey};

pub use bandwidth::BandwidthThrottles;

mod accept;
mod agents;
mod bloom;
mod initiate;
mod ops;
mod state_map;
mod store;

mod bandwidth;
mod metrics;
mod next_target;

#[cfg(test)]
mod tests;

/// max send buffer size (keep it under 16384 with a little room for overhead)
/// (this is not a tuning_param because it must be coordinated
/// with the constant in PoolBuf which cannot be set at runtime)
const MAX_SEND_BUF_BYTES: usize = 16000;

/// The maximum number of different nodes that will be
/// gossiped with if gossip is triggered.
const MAX_TRIGGERS: u8 = 2;

/// The timeout for a gossip round if there is no contact. Five minutes.
const ROUND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60 * 5);

type BloomFilter = bloomfilter::Bloom<Arc<MetaOpKey>>;
type EventSender = futures::channel::mpsc::Sender<event::KitsuneP2pEvent>;

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
#[derive(Debug, Clone, Copy)]
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
    inner: Share<ShardedGossipState>,
    /// Bandwidth for incoming and outgoing gossip.
    bandwidth: Arc<BandwidthThrottle>,
}

/// Basic statistic for gossip loop processing performance.
struct Stats {
    start: std::time::Instant,
    avg_processing_time: std::time::Duration,
    max_processing_time: std::time::Duration,
    count: u32,
}

impl Stats {
    /// Reset the stats.
    fn reset() -> Self {
        Stats {
            start: std::time::Instant::now(),
            avg_processing_time: std::time::Duration::default(),
            max_processing_time: std::time::Duration::default(),
            count: 0,
        }
    }
}

impl ShardedGossip {
    /// Constructor
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: EventSender,
        gossip_type: GossipType,
        bandwidth: Arc<BandwidthThrottle>,
    ) -> Arc<Self> {
        let this = Arc::new(Self {
            ep_hnd,
            inner: Share::new(Default::default()),
            gossip: ShardedGossipLocal {
                tuning_params,
                space,
                evt_sender,
                inner: Share::new(ShardedGossipLocalState::default()),
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
        let (_endpoint, how, gossip) = outgoing;
        let s = tracing::trace_span!("process_outgoing", cert = ?_endpoint.cert(), agents = ?self.gossip.show_local_agents());
        s.in_scope(|| tracing::trace!(?gossip));
        let gossip = gossip.encode_vec().map_err(KitsuneError::other)?;
        let bytes = gossip.len();
        let gossip = wire::Wire::gossip(
            self.gossip.space.clone(),
            gossip.into(),
            self.gossip.gossip_type.into(),
        );

        let timeout = self.gossip.tuning_params.implicit_timeout();

        let con = match how {
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
        self.bandwidth.outgoing_bytes(bytes).await;
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
                    self.gossip.remove_state(&con.peer_cert(), true).await?;
                    vec![ShardedGossipWire::error(e.to_string())]
                }
            };
            self.inner.share_mut(|i, _| {
                i.outgoing.extend(outgoing.into_iter().map(|msg| {
                    (
                        GossipTgt::new(Vec::with_capacity(0), con.peer_cert()),
                        HowToConnect::Con(con.clone(), remote_url.clone()),
                        msg,
                    )
                }));
                Ok(())
            })?;
        }
        if let Some(outgoing) = outgoing {
            let cert = outgoing.0.cert().clone();
            if let Err(err) = self.process_outgoing(outgoing).await {
                self.gossip.remove_state(&cert, true).await?;
                tracing::error!(
                    "Gossip failed to send outgoing message because of: {:?}",
                    err
                );
            }
        }

        if self.gossip.should_local_sync()? {
            self.gossip.local_sync().await?;
        }

        Ok(())
    }

    async fn run_one_iteration(&self) {
        match self.gossip.try_initiate().await {
            Ok(Some(outgoing)) => {
                if let Err(err) = self.inner.share_mut(|i, _| {
                    i.outgoing.push_back(outgoing);
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
        self.inner.share_mut(move |inner, _| {
            let incoming = inner.incoming.pop_front();
            let outgoing = inner.outgoing.pop_front();
            Ok((incoming, outgoing))
        })
    }

    /// Log the statistics for the gossip loop.
    fn stats(&self, stats: &mut Stats) {
        if let GossipType::Recent = self.gossip.gossip_type {
            let elapsed = stats.start.elapsed();
            stats.avg_processing_time += elapsed;
            stats.max_processing_time = std::cmp::max(stats.max_processing_time, elapsed);
            stats.count += 1;
            if elapsed.as_secs() > 5 {
                stats.avg_processing_time = stats
                    .avg_processing_time
                    .checked_div(stats.count)
                    .unwrap_or_default();
                let _ = self.gossip.inner.share_mut(|i, _| {
                    let s = tracing::trace_span!("gossip_metrics");
                    s.in_scope(|| tracing::trace!("{}\nStats over last 5s:\n\tAverage processing time {:?}\n\tIteration count: {}\n\tMax gossip processing time: {:?}", i.metrics, stats.avg_processing_time, stats.count, stats.max_processing_time));
                    Ok(())
                });
                *stats = Stats::reset();
            }
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
    inner: Share<ShardedGossipLocalState>,
    closing: AtomicBool,
}

/// Incoming gossip.
type Incoming = (Tx2ConHnd<wire::Wire>, TxUrl, ShardedGossipWire, usize);
/// Outgoing gossip.
type Outgoing = (GossipTgt, HowToConnect, ShardedGossipWire);

type StateKey = Tx2Cert;

/// The internal mutable state for [`ShardedGossipLocal`]
#[derive(Default)]
pub struct ShardedGossipLocalState {
    /// The list of agents on this node
    local_agents: HashSet<Arc<KitsuneAgent>>,
    /// If Some, we are in the process of trying to initiate gossip with this target.
    initiate_tgt: Option<(GossipTgt, u32)>,
    round_map: RoundStateMap,
    /// Metrics that track remote node states and help guide
    /// the next node to gossip with.
    metrics: Metrics,
    #[allow(dead_code)]
    /// Last moment we locally synced.
    last_local_sync: Option<std::time::Instant>,
    /// Trigger local sync to run on the next iteration.
    trigger_local_sync: bool,
}

impl ShardedGossipLocalState {
    fn remove_state(&mut self, state_key: &StateKey, error: bool) -> Option<RoundState> {
        // Check if the round to be removed matches the current initiate_tgt
        let init_tgt = self
            .initiate_tgt
            .as_ref()
            .map(|tgt| tgt.0.cert() == state_key)
            .unwrap_or(false);
        if init_tgt {
            self.initiate_tgt = None;
        }
        let r = self.round_map.remove(state_key);
        if r.is_some() {
            if error {
                self.metrics.record_error(state_key.clone());
            } else {
                self.metrics.record_success(state_key.clone());
            }
        } else if init_tgt && error {
            self.metrics.record_error(state_key.clone());
        }
        r
    }

    fn check_tgt_expired(&mut self) {
        if let Some(cert) = self.initiate_tgt.as_ref().map(|tgt| tgt.0.cert().clone()) {
            if self.round_map.check_timeout(&cert) {
                self.initiate_tgt = None;
            }
        }
    }

    fn new_integrated_data(&mut self) -> KitsuneResult<()> {
        let s = tracing::trace_span!("gossip_trigger", agents = ?self.show_local_agents());
        s.in_scope(|| self.log_state());
        self.metrics.record_force_initiate();
        self.trigger_local_sync = true;
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

/// The internal mutable state for [`ShardedGossip`]
#[derive(Default)]
pub struct ShardedGossipState {
    incoming: VecDeque<Incoming>,
    outgoing: VecDeque<Outgoing>,
}

/// The state representing a single active ongoing "round" of gossip with a
/// remote node
#[derive(Debug, Clone)]
pub struct RoundState {
    /// The common ground with our gossip partner for the purposes of this round
    common_arc_set: Arc<DhtArcSet>,
    /// Number of ops blooms we have sent for this round, which is also the
    /// number of MissingOps sets we expect in response
    num_sent_ops_blooms: u8,
    /// We've received the last op bloom filter from our partner
    /// (the one with `finished` == true)
    received_all_incoming_ops_blooms: bool,
    /// Round start time
    created_at: std::time::Instant,
    /// Last moment we had any contact for this round.
    last_touch: std::time::Instant,
    /// Amount of time before a round is considered expired.
    round_timeout: std::time::Duration,
}

impl ShardedGossipLocal {
    const TGT_FP: f64 = 0.01;
    /// This should give us just under 16MB for the bloom filter.
    /// Based on a compression of 75%.
    const UPPER_HASHES_BOUND: usize = 500;

    /// Calculate the time range for a gossip round.
    fn calculate_time_ranges(&self) -> Vec<TimeWindow> {
        const NOW: Duration = Duration::from_secs(0);
        const HOUR: Duration = Duration::from_secs(60 * 60);
        const DAY: Duration = Duration::from_secs(60 * 60 * 24);
        const WEEK: Duration = Duration::from_secs(60 * 60 * 24 * 7);
        const MONTH: Duration = Duration::from_secs(60 * 60 * 24 * 7 * 30);
        const START_OF_TIME: Duration = Duration::MAX;
        match self.gossip_type {
            GossipType::Recent => {
                vec![time_range(HOUR, NOW)]
            }
            GossipType::Historical => {
                vec![
                    time_range(DAY, HOUR),
                    time_range(WEEK, DAY),
                    time_range(MONTH, WEEK),
                    time_range(START_OF_TIME, MONTH),
                ]
            }
        }
    }

    fn new_state(&self, common_arc_set: Arc<DhtArcSet>) -> KitsuneResult<RoundState> {
        Ok(RoundState {
            common_arc_set,
            num_sent_ops_blooms: 0,
            received_all_incoming_ops_blooms: false,
            created_at: std::time::Instant::now(),
            last_touch: std::time::Instant::now(),
            round_timeout: ROUND_TIMEOUT,
        })
    }

    async fn get_state(&self, id: &StateKey) -> KitsuneResult<Option<RoundState>> {
        self.inner
            .share_mut(|i, _| Ok(i.round_map.get(id).cloned()))
    }

    async fn remove_state(&self, id: &StateKey, error: bool) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| Ok(i.remove_state(id, error)))
    }

    async fn remove_target(&self, id: &StateKey, error: bool) -> KitsuneResult<()> {
        self.inner.share_mut(|i, _| {
            if i.initiate_tgt
                .as_ref()
                .map(|tgt| tgt.0.cert() == id)
                .unwrap_or(false)
            {
                i.initiate_tgt = None;
                if error {
                    i.metrics.record_error(id.clone());
                } else {
                    i.metrics.record_success(id.clone());
                }
            }
            Ok(())
        })
    }

    async fn incoming_ops_finished(
        &self,
        state_id: &StateKey,
    ) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| {
            let finished = i
                .round_map
                .get_mut(state_id)
                .map(|state| {
                    state.received_all_incoming_ops_blooms = true;
                    state.num_sent_ops_blooms == 0
                })
                .unwrap_or(true);
            if finished {
                Ok(i.remove_state(state_id, false))
            } else {
                Ok(i.round_map.get(state_id).cloned())
            }
        })
    }

    async fn decrement_ops_blooms(&self, state_id: &StateKey) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| {
            let update_state = |state: &mut RoundState| {
                let num_ops_blooms = state.num_sent_ops_blooms.saturating_sub(1);
                state.num_sent_ops_blooms = num_ops_blooms;
                state.num_sent_ops_blooms == 0 && state.received_all_incoming_ops_blooms
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
        let s = tracing::trace_span!("process_incoming", ?cert, agents = ?self.show_local_agents(), ?msg);
        s.in_scope(|| self.log_state());
        // If we don't have the state for a message then the other node will need to timeout.
        Ok(match msg {
            ShardedGossipWire::Initiate(Initiate { intervals, id }) => {
                self.incoming_initiate(cert, intervals, id).await?
            }
            ShardedGossipWire::Accept(Accept { intervals }) => {
                self.incoming_accept(cert, intervals).await?
            }
            ShardedGossipWire::Agents(Agents { filter }) => {
                if let Some(state) = self.get_state(&cert).await? {
                    let filter = decode_bloom_filter(&filter);
                    self.incoming_agents(state, filter).await?
                } else {
                    Vec::with_capacity(0)
                }
            }
            ShardedGossipWire::MissingAgents(MissingAgents { agents }) => {
                if let Some(state) = self.get_state(&cert).await? {
                    self.incoming_missing_agents(state, agents.as_slice())
                        .await?;
                }
                Vec::with_capacity(0)
            }
            ShardedGossipWire::Ops(Ops {
                missing_hashes,
                finished,
            }) => {
                let state = if finished {
                    self.incoming_ops_finished(&cert).await?
                } else {
                    self.get_state(&cert).await?
                };
                match state {
                    Some(state) => match missing_hashes {
                        EncodedTimedBloomFilter::NoOverlap => Vec::with_capacity(0),
                        EncodedTimedBloomFilter::MissingAllHashes { time_window } => {
                            let filter = TimedBloomFilter {
                                bloom: None,
                                time: time_window,
                            };
                            self.incoming_ops(state, filter).await?
                        }
                        EncodedTimedBloomFilter::HaveHashes {
                            filter,
                            time_window,
                        } => {
                            let filter = TimedBloomFilter {
                                bloom: Some(decode_bloom_filter(&filter)),
                                time: time_window,
                            };
                            self.incoming_ops(state, filter).await?
                        }
                    },
                    None => Vec::with_capacity(0),
                }
            }
            ShardedGossipWire::MissingOps(MissingOps { ops, finished }) => {
                let state = if finished {
                    self.decrement_ops_blooms(&cert).await?
                } else {
                    self.get_state(&cert).await?
                };
                if let Some(state) = state {
                    self.incoming_missing_ops(state, ops).await?;
                }
                Vec::with_capacity(0)
            }
            ShardedGossipWire::NoAgents(_) => {
                self.remove_state(&cert, true).await?;
                Vec::with_capacity(0)
            }
            ShardedGossipWire::AlreadyInProgress(_) => {
                self.remove_target(&cert, false).await?;
                Vec::with_capacity(0)
            }
            ShardedGossipWire::Error(Error { message }) => {
                tracing::warn!("gossiping with: {:?} and got error: {}", cert, message);
                self.remove_state(&cert, true).await?;
                Vec::with_capacity(0)
            }
        })
    }

    async fn local_sync(&self) -> KitsuneResult<()> {
        let local_agents = self.inner.share_mut(|i, _| Ok(i.local_agents.clone()))?;
        let agent_arcs =
            store::local_agent_arcs(&self.evt_sender, &self.space, &local_agents).await?;
        let arcs: Vec<_> = agent_arcs.iter().map(|(_, arc)| arc.clone()).collect();
        let arcset = local_sync_arcset(arcs.as_slice());
        let op_hashes = store::all_op_hashes_within_arcset(
            &self.evt_sender,
            &self.space,
            agent_arcs.as_slice(),
            &arcset,
            full_time_window(),
            usize::MAX,
            true,
        )
        .await?
        .map(|(ops, _window)| ops)
        .unwrap_or_default();

        let ops: Vec<_> = store::fetch_ops(
            &self.evt_sender,
            &self.space,
            local_agents.iter(),
            op_hashes,
        )
        .await?
        .into_iter()
        .collect();

        store::put_ops(&self.evt_sender, &self.space, agent_arcs, ops).await?;
        Ok(())
    }

    /// Check if we should locally sync
    fn should_local_sync(&self) -> KitsuneResult<bool> {
        // Historical gossip should not locally sync.
        if matches!(self.gossip_type, GossipType::Historical)
            || self.tuning_params.gossip_single_storage_arc_per_space
        {
            return Ok(false);
        }
        let update_last_sync = |i: &mut ShardedGossipLocalState, _: &mut bool| {
            if i.local_agents.len() < 2 {
                Ok(false)
            } else if i.trigger_local_sync {
                // We are force triggering a local sync.
                i.trigger_local_sync = false;
                i.last_local_sync = Some(std::time::Instant::now());
                let s = tracing::trace_span!("trigger",agents = ?i.show_local_agents(), i.trigger_local_sync);
                s.in_scope(|| tracing::trace!("Force local sync"));
                Ok(true)
            } else if i
                .last_local_sync
                .as_ref()
                .map(|s| s.elapsed().as_millis() as u32)
                .unwrap_or(u32::MAX)
                >= self.tuning_params.gossip_local_sync_delay_ms
            {
                // It's been long enough since the last local sync.
                i.last_local_sync = Some(std::time::Instant::now());
                Ok(true)
            } else {
                // Otherwise it's not time to sync.
                Ok(false)
            }
        };

        self.inner.share_mut(update_last_sync)
    }

    /// Record all timed out rounds into metrics
    fn record_timeouts(&self) {
        self.inner
            .share_mut(|i, _| {
                for cert in i.round_map.take_timed_out_rounds() {
                    i.metrics.record_error(cert);
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

/// Calculates the arcset used during local sync. This arcset determines the
/// minimum set of ops to be spread across all local agents in order to reach
/// local consistency
fn local_sync_arcset(arcs: &[ArcInterval]) -> DhtArcSet {
    arcs.iter()
        .enumerate()
        // For each agent's arc,
        .map(|(i, arc_i)| {
            // find the union of all arcs *other* than this one,
            let other_arcset = arcs
                .iter()
                .enumerate()
                .filter_map(|(j, arc_j)| {
                    if i == j {
                        None
                    } else {
                        Some(DhtArcSet::from(arc_j))
                    }
                })
                .fold(DhtArcSet::new_empty(), |a, b| DhtArcSet::union(&a, &b));
            // and return the intersection of this arc with the union of the others.
            DhtArcSet::from(arc_i).intersection(&other_arcset)
        })
        // and take the union of all of the intersections
        .fold(DhtArcSet::new_empty(), |a, b| DhtArcSet::union(&a, &b))
}

impl RoundState {
    fn increment_sent_ops_blooms(&mut self) -> u8 {
        self.num_sent_ops_blooms += 1;
        self.num_sent_ops_blooms
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

kitsune_p2p_types::write_codec_enum! {
    /// SimpleBloom Gossip Wire Protocol Codec
    codec ShardedGossipWire {
        /// Initiate a round of gossip with a remote node
        Initiate(0x10) {
            /// The list of arc intervals (equivalent to a [`DhtArcSet`])
            /// for all local agents
            intervals.0: Vec<ArcInterval>,
            /// A random number to resolve concurrent initiates.
            id.1: u32,
        },

        /// Accept an incoming round of gossip from a remote node
        Accept(0x20) {
            /// The list of arc intervals (equivalent to a [`DhtArcSet`])
            /// for all local agents
            intervals.0: Vec<ArcInterval>,
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

        /// Send Ops Bloom
        Ops(0x50) {
            /// The bloom filter for op data
            missing_hashes.0: EncodedTimedBloomFilter,
            /// Is this the last bloom to be sent?
            finished.1: bool,
        },

        /// Any ops that were missing from the remote bloom.
        MissingOps(0x60) {
            /// The missing ops
            ops.0: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
            /// Is this the last chunk of ops to be sent in response
            /// to the bloom filter that we're responding to?
            finished.1: bool,
        },

        /// The node you are trying to gossip with has no agents anymore.
        NoAgents(0x80) {
        },

        /// You have sent a stale initiate to a node
        /// that already has an active round with you.
        AlreadyInProgress(0x90) {
        },

        /// The node you are gossiping with has hit an error condition
        /// and failed to respond to a request.
        Error(0x11) {
            /// The error message.
            message.0: String,
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
        self.inner.share_mut(move |i, _| {
            i.incoming
                .push_back((con, remote_url, gossip, bytes as usize));
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
    ) -> GossipModule {
        GossipModule(ShardedGossip::new(
            tuning_params,
            space,
            ep_hnd,
            evt_sender,
            GossipType::Recent,
            self.bandwidth.clone(),
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
    ) -> GossipModule {
        GossipModule(ShardedGossip::new(
            tuning_params,
            space,
            ep_hnd,
            evt_sender,
            GossipType::Historical,
            self.bandwidth.clone(),
        ))
    }
}

/// Create a recent [`GossipModuleFactory`]
pub fn recent_factory(bandwidth: Arc<BandwidthThrottle>) -> GossipModuleFactory {
    GossipModuleFactory(Arc::new(ShardedRecentGossipFactory::new(bandwidth)))
}

/// Create a [`GossipModuleFactory`]
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
