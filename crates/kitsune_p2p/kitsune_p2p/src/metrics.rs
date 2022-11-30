//! metrics tracked by kitsune_p2p spaces

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use crate::gossip::sharded_gossip::NodeId;
use crate::gossip::sharded_gossip::RegionDiffs;
use crate::gossip::sharded_gossip::RoundState;
use crate::gossip::sharded_gossip::RoundThroughput;
use crate::types::event::*;
use crate::types::*;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::agent_info::AgentInfoSigned;

use num_traits::*;

/// how long historical metric records should be kept
/// (currently set to 1 week)
const HISTORICAL_RECORD_EXPIRE_DURATION_MICROS: i64 = 1000 * 1000 * 60 * 60 * 24 * 7;

/// Running average that prioritizes memory and cpu efficiency
/// over strict accuracy.
/// For metrics where we can't afford the memory of tracking samples
/// for every remote we might talk to, this running average is
/// accurate enough and uses only 5 bytes of memory.
#[derive(Debug, Clone, Copy)]
pub struct RunAvg(f32, u8);

impl Default for RunAvg {
    fn default() -> Self {
        Self(0.0, 0)
    }
}

impl RunAvg {
    /// Push a new data point onto the running average
    pub fn push<V: AsPrimitive<f32>>(&mut self, v: V) {
        self.push_n(v, 1);
    }

    /// Push multiple entries (up to 255) of the same value onto the average
    pub fn push_n<V: AsPrimitive<f32>>(&mut self, v: V, count: u8) {
        self.1 = self.1.saturating_add(count);
        self.0 = (self.0 * (self.1 - count) as f32 + (v.as_() * count as f32)) / self.1 as f32;
    }
}

macro_rules! mk_from {
    ($($t:ty,)*) => {$(
        impl From<$t> for RunAvg {
            fn from(o: $t) -> Self {
                Self(o as f32, 1)
            }
        }
    )*};
}

mk_from! {
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    i64,
    u64,
    f32,
    f64,
}

impl std::ops::Deref for RunAvg {
    type Target = f32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<f32> for RunAvg {
    fn as_ref(&self) -> &f32 {
        &self.0
    }
}

impl std::borrow::Borrow<f32> for RunAvg {
    fn borrow(&self) -> &f32 {
        &self.0
    }
}

impl std::fmt::Display for RunAvg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The maximum number of different nodes that will be
/// gossiped with if gossip is triggered.
const MAX_TRIGGERS: u8 = 2;

/// Maximum amount of history we will track
/// per remote node.
const MAX_HISTORY: usize = 10;

#[derive(Debug, Clone, Default)]
/// The history of gossip with an agent on a remote node.
/// We record metrics per agent,
pub struct PeerAgentHistory {
    /// Sucessful and unsuccessful messages from the remote
    /// can be combined to estimate a "reachability quotient"
    /// between 1 (or 0 if empty) and 100. Errors are weighted
    /// heavier because we retry less frequently.
    pub reachability_quotient: RunAvg,
    /// Running average for latency microseconds for any direct
    /// request/response calls to remote agent.
    pub latency_micros: RunAvg,
    /// Times we recorded successful initiates to this node (they accepted).
    pub initiates: VecDeque<RoundMetric>,
    /// Times we recorded initates from this node (we accepted).
    pub accepts: VecDeque<RoundMetric>,
    /// Times we recorded complete rounds for this node.
    pub successes: VecDeque<RoundMetric>,
    /// Times we recorded errors for this node.
    pub errors: VecDeque<RoundMetric>,
    /// Is this node currently in an active round?
    pub current_round: bool,
}

/// Detailed info about the history of gossip with this node
#[derive(Debug, Clone, Default)]
pub struct PeerNodeHistory {
    /// The most recent list of remote agents reported by this node
    pub remote_agents: Vec<Arc<KitsuneAgent>>,

    /// Detailed info about the ongoing round with this node
    pub current_round: Option<CurrentRound>,

    /// Detailed info about rounds completed with this node
    pub completed_rounds: VecDeque<CompletedRound>,
}

/// Info about a completed gossip round
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundMetric {
    /// The time this metric was recorded
    pub instant: Instant,
    /// The type of gossip module
    pub gossip_type: GossipModuleType,
}

/// Metrics about a completed gossip round
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedRound {
    /// Unique string id
    pub id: String,
    /// The type of gossip module
    pub gossip_type: GossipModuleType,
    /// The start time of the round
    pub start_time: Instant,
    /// The end time of the round
    pub end_time: Instant,
    /// Throughput stats
    pub throughput: RoundThroughput,
    /// This round ended in an error
    pub error: bool,
    /// If historical, the region diffs
    pub region_diffs: RegionDiffs,
}

impl CompletedRound {
    /// Total duration of this round, from start to end
    pub fn duration(&self) -> Duration {
        self.end_time.duration_since(self.start_time)
    }
}

/// Metrics about an ongoing gossip round
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentRound {
    /// Unique string id
    pub id: String,
    /// The type of gossip module
    pub gossip_type: GossipModuleType,
    /// Last time this was updated
    pub last_touch: Instant,
    /// The start time of the round
    pub start_time: Instant,
    /// Total information sent/received so far
    pub throughput: RoundThroughput,
    /// If historical, the region diffs
    pub region_diffs: RegionDiffs,
}

impl CurrentRound {
    /// Constructor
    pub fn new(id: String, gossip_type: GossipModuleType, start_time: Instant) -> Self {
        Self {
            id,
            gossip_type,
            start_time,
            last_touch: Instant::now(),
            throughput: Default::default(),
            region_diffs: Default::default(),
        }
    }

    /// Update status based on an existing round
    pub fn update(&mut self, round_state: &RoundState) {
        self.last_touch = Instant::now();
        self.throughput = round_state.throughput.clone();
        self.region_diffs = round_state.region_diffs.clone();
    }

    /// Convert to a CompletedRound
    pub fn completed(self, error: bool) -> CompletedRound {
        CompletedRound {
            id: self.id,
            gossip_type: self.gossip_type,
            start_time: self.start_time,
            end_time: Instant::now(),
            throughput: self.throughput,
            error,
            region_diffs: self.region_diffs,
        }
    }
}

impl RoundMetric {
    /// Time elapsed since this round was recorded
    pub fn elapsed(&self) -> Duration {
        self.instant.elapsed()
    }
}

impl PartialOrd for RoundMetric {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RoundMetric {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.instant.cmp(&other.instant)
    }
}

impl PartialOrd for CompletedRound {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CompletedRound {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.start_time.cmp(&other.start_time) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        self.end_time.cmp(&other.end_time)
    }
}

impl PartialOrd for CurrentRound {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CurrentRound {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.start_time.cmp(&other.start_time)
    }
}

#[derive(Debug, Default)]
/// Metrics tracking for remote nodes to help
/// choose which remote node to initiate the next round with.
pub struct Metrics {
    /// Map of remote agents and gossip history with each.
    agent_history: HashMap<Arc<KitsuneAgent>, PeerAgentHistory>,

    /// Map of remote nodes and gossip history with each
    node_history: HashMap<NodeId, PeerNodeHistory>,

    /// Aggregate Extrapolated Dht Coverage
    agg_extrap_cov: RunAvg,

    // Number of times we need to force initiate
    // the next round.
    pub(crate) force_initiates: u8,
}

/// Outcome of a gossip round.
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum RoundOutcome {
    /// Success outcome
    Success(RoundMetric),
    /// Error outcome
    Error(RoundMetric),
}

/// Accept differing key types
pub enum AgentLike<'lt> {
    /// An agent info
    Info(&'lt AgentInfoSigned),
    /// A raw agent pubkey
    PubKey(&'lt Arc<KitsuneAgent>),
}

impl<'lt> From<&'lt AgentInfoSigned> for AgentLike<'lt> {
    fn from(i: &'lt AgentInfoSigned) -> Self {
        Self::Info(i)
    }
}

impl<'lt> From<&'lt Arc<KitsuneAgent>> for AgentLike<'lt> {
    fn from(pk: &'lt Arc<KitsuneAgent>) -> Self {
        Self::PubKey(pk)
    }
}

impl<'lt> AgentLike<'lt> {
    /// Get a raw agent pubkey from any variant type
    pub fn agent(&self) -> &Arc<KitsuneAgent> {
        match self {
            Self::Info(i) => &i.agent,
            Self::PubKey(pk) => pk,
        }
    }
}

impl Metrics {
    /// Dump historical metrics for recording to db.
    pub fn dump_historical(&self) -> Vec<MetricRecord> {
        let now = Timestamp::now();

        let expires_at =
            Timestamp::from_micros(now.as_micros() + HISTORICAL_RECORD_EXPIRE_DURATION_MICROS);

        let mut out = Vec::new();

        for (agent, node) in self.agent_history.iter() {
            out.push(MetricRecord {
                kind: MetricRecordKind::ReachabilityQuotient,
                agent: Some(agent.clone()),
                recorded_at_utc: now,
                expires_at_utc: expires_at,
                data: serde_json::json!(*node.reachability_quotient),
            });

            out.push(MetricRecord {
                kind: MetricRecordKind::LatencyMicros,
                agent: Some(agent.clone()),
                recorded_at_utc: now,
                expires_at_utc: expires_at,
                data: serde_json::json!(*node.latency_micros),
            });
        }

        out.push(MetricRecord {
            kind: MetricRecordKind::AggExtrapCov,
            agent: None,
            recorded_at_utc: now,
            expires_at_utc: expires_at,
            data: serde_json::json!(*self.agg_extrap_cov),
        });

        out
    }

    /// Dump json encoded metrics
    pub fn dump(&self) -> serde_json::Value {
        let agents: serde_json::Value = self
            .agent_history
            .iter()
            .map(|(a, i)| {
                (
                    a.to_string(),
                    serde_json::json!({
                        "reachability_quotient": *i.reachability_quotient,
                        "latency_micros": *i.latency_micros,
                    }),
                )
            })
            .collect::<serde_json::map::Map<String, serde_json::Value>>()
            .into();

        serde_json::json!({
            "aggExtrapCov": *self.agg_extrap_cov,
            "agents": agents,
        })
    }

    /// Get the sum of throughputs for all current rounds
    pub fn current_throughputs(
        &self,
        gossip_type: GossipModuleType,
    ) -> impl Iterator<Item = RoundThroughput> + '_ {
        self.node_history
            .values()
            .flat_map(|r| &r.current_round)
            .filter(move |r| r.gossip_type == gossip_type)
            .map(|r| r.throughput.clone())
    }

    /// Get the sum of throughputs for all current rounds
    pub fn total_current_historical_throughput(&self) -> RoundThroughput {
        self.current_throughputs(GossipModuleType::ShardedHistorical)
            .sum()
    }

    /// Record an individual extrapolated coverage event
    /// (either from us or a remote)
    /// and add it to our running aggregate extrapolated coverage metric.
    pub fn record_extrap_cov_event(&mut self, extrap_cov: f32) {
        self.agg_extrap_cov.push(extrap_cov);
    }

    /// Sucessful and unsuccessful messages from the remote
    /// can be combined to estimate a "reachability quotient"
    /// between 1 (or 0 if empty) and 100. Errors are weighted
    /// heavier because we retry less frequently.
    /// Call this to register a reachability event.
    /// Note, `record_success` and `record_error` below invoke this
    /// function internally, you don't need to call it again.
    pub fn record_reachability_event<'a, T, I>(&mut self, success: bool, remote_agent_list: I)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        for agent_info in remote_agent_list {
            let info = self
                .agent_history
                .entry(agent_info.into().agent().clone())
                .or_default();
            if success {
                info.reachability_quotient.push(100);
            } else {
                info.reachability_quotient.push_n(1, 5);
            }
        }
    }

    /// Running average for latency microseconds for any direct
    /// request/response calls to remote agent.
    pub fn record_latency_micros<'a, T, I, V>(&mut self, micros: V, remote_agent_list: I)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
        V: AsPrimitive<f32>,
    {
        for agent_info in remote_agent_list {
            let history = self
                .agent_history
                .entry(agent_info.into().agent().clone())
                .or_default();
            history.latency_micros.push(micros);
        }
    }

    /// Record a gossip round has been initiated by us.
    pub fn record_initiate<'a, T, I>(&mut self, remote_agent_list: I, gossip_type: GossipModuleType)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        for agent_info in remote_agent_list {
            let history = self
                .agent_history
                .entry(agent_info.into().agent().clone())
                .or_default();
            let round = RoundMetric {
                instant: Instant::now(),
                gossip_type,
            };
            record_item(&mut history.initiates, round);
            if history.current_round {
                tracing::warn!("Recorded initiate with current round already set");
            }
            history.current_round = true;
        }
    }

    /// Record a gossip round has been initiated by a peer.
    pub fn record_accept<'a, T, I>(&mut self, remote_agent_list: I, gossip_type: GossipModuleType)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        for agent_info in remote_agent_list {
            let history = self
                .agent_history
                .entry(agent_info.into().agent().clone())
                .or_default();
            let round = RoundMetric {
                instant: Instant::now(),
                gossip_type,
            };
            record_item(&mut history.accepts, round);
            if history.current_round {
                tracing::warn!("Recorded accept with current round already set");
            }
            history.current_round = true;
        }
    }

    /// Record a gossip round has completed successfully.
    pub fn record_success<'a, T, I>(&mut self, remote_agent_list: I, gossip_type: GossipModuleType)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        let mut should_dec_force_initiates = false;

        for agent_info in remote_agent_list {
            let history = self
                .agent_history
                .entry(agent_info.into().agent().clone())
                .or_default();
            history.reachability_quotient.push(100);
            let round = RoundMetric {
                instant: Instant::now(),
                gossip_type,
            };
            record_item(&mut history.successes, round);
            history.current_round = false;
            if history.is_initiate_round() {
                should_dec_force_initiates = true;
            }
        }

        if should_dec_force_initiates {
            self.force_initiates = self.force_initiates.saturating_sub(1);
        }

        tracing::debug!(
            "recorded success in metrics. force_initiates={}",
            self.force_initiates
        );
    }

    /// Record a gossip round has finished with an error.
    pub fn record_error<'a, T, I>(&mut self, remote_agent_list: I, gossip_type: GossipModuleType)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        for agent_info in remote_agent_list {
            let history = self
                .agent_history
                .entry(agent_info.into().agent().clone())
                .or_default();
            history.reachability_quotient.push_n(1, 5);
            let round = RoundMetric {
                instant: Instant::now(),
                gossip_type,
            };
            record_item(&mut history.errors, round);
            history.current_round = false;
        }
        tracing::debug!(
            "recorded error in metrics. force_initiates={}",
            self.force_initiates
        );
    }

    /// Update node-level info about a current round, or create one if it doesn't exist
    pub fn update_current_round(
        &mut self,
        peer: &NodeId,
        gossip_type: GossipModuleType,
        round_state: &RoundState,
    ) {
        let remote_agents = round_state
            .remote_agent_list
            .clone()
            .into_iter()
            .map(|a| a.agent())
            .collect();
        let history = self.node_history.entry(peer.clone()).or_default();
        history.remote_agents = remote_agents;
        if let Some(r) = &mut history.current_round {
            r.update(round_state);
        } else {
            history.current_round = Some(CurrentRound::new(
                round_state.id.clone(),
                gossip_type,
                Instant::now(),
            ));
        }

        // print progress
        {
            let tps = self
                .current_throughputs(GossipModuleType::ShardedHistorical)
                .count();
            let tot = self.total_current_historical_throughput();
            let n = tot.op_bytes.incoming;
            let d = tot.expected_op_bytes.incoming;
            if d > 0 {
                let r = n as f64 / d as f64 * 100.0;
                tracing::debug!(
                    "PROGRESS [{:?}] {} / {} ({:>3.1}%) : {}",
                    peer,
                    n,
                    d,
                    r,
                    tps,
                );
            }
        }
    }

    /// Remove the current round info once it's complete, and put it into the history list
    pub fn complete_current_round(&mut self, node: &NodeId, error: bool) {
        let history = self.node_history.entry(node.clone()).or_default();
        let r = history.current_round.take();
        if let Some(r) = r {
            history.completed_rounds.push_back(r.completed(error))
        }
    }

    /// Record that we should force initiate the next few rounds.
    pub fn record_force_initiate(&mut self) {
        self.force_initiates = MAX_TRIGGERS;
    }

    /// Get the last successful round time.
    pub fn last_success<'a, T, I>(&self, remote_agent_list: I) -> Option<&RoundMetric>
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        remote_agent_list
            .into_iter()
            .filter_map(|agent_info| self.agent_history.get(agent_info.into().agent()))
            .filter_map(|info| info.successes.back())
            .min_by_key(|r| r.instant)
    }

    /// Is this node currently in an active round?
    pub fn is_current_round<'a, T, I>(&self, remote_agent_list: I) -> bool
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        remote_agent_list
            .into_iter()
            .filter_map(|agent_info| self.agent_history.get(agent_info.into().agent()))
            .any(|info| info.current_round)
    }

    /// What was the last outcome for this node's gossip round?
    pub fn last_outcome<'a, T, I>(&self, remote_agent_list: I) -> Option<RoundOutcome>
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        #[allow(clippy::map_flatten)]
        remote_agent_list
            .into_iter()
            .filter_map(|agent_info| self.agent_history.get(agent_info.into().agent()))
            .map(|info| {
                [
                    info.errors.back().map(|x| RoundOutcome::Error(x.clone())),
                    info.successes
                        .back()
                        .map(|x| RoundOutcome::Success(x.clone())),
                ]
            })
            .flatten()
            .flatten()
            .max()
    }

    /// Should we force initiate the next round?
    pub fn forced_initiate(&self) -> bool {
        self.force_initiates > 0
    }

    /// Return the average (mean) reachability quotient for the
    /// supplied remote agents.
    pub fn reachability_quotient<'a, T, I>(&self, remote_agent_list: I) -> f32
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        let (sum, cnt) = remote_agent_list
            .into_iter()
            .filter_map(|agent_info| self.agent_history.get(agent_info.into().agent()))
            .map(|info| *info.reachability_quotient)
            .fold((0.0, 0.0), |acc, x| (acc.0 + x, acc.1 + 1.0));
        if cnt <= 0.0 {
            0.0
        } else {
            sum / cnt
        }
    }

    /// Return the average (mean) latency microseconds for the
    /// supplied remote agents.
    pub fn latency_micros<'a, T, I>(&self, remote_agent_list: I) -> f32
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        let (sum, cnt) = remote_agent_list
            .into_iter()
            .filter_map(|agent_info| self.agent_history.get(agent_info.into().agent()))
            .map(|info| *info.latency_micros)
            .fold((0.0, 0.0), |acc, x| (acc.0 + x, acc.1 + 1.0));
        if cnt <= 0.0 {
            0.0
        } else {
            sum / cnt
        }
    }

    /// Getter
    pub fn peer_agent_histories(&self) -> &HashMap<Arc<KitsuneAgent>, PeerAgentHistory> {
        &self.agent_history
    }

    /// Getter
    pub fn peer_node_histories(&self) -> &HashMap<NodeId, PeerNodeHistory> {
        &self.node_history
    }
}

impl PeerAgentHistory {
    /// Was the last round for this node initiated by us?
    fn is_initiate_round(&self) -> bool {
        match (self.accepts.back(), self.initiates.back()) {
            (None, None) | (Some(_), None) => false,
            (None, Some(_)) => true,
            (Some(remote), Some(initiate)) => initiate > remote,
        }
    }
}

fn record_item<T>(buffer: &mut VecDeque<T>, item: T) {
    if buffer.len() > MAX_HISTORY {
        buffer.pop_front();
    }
    buffer.push_back(item);
}

impl std::fmt::Display for Metrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        static TRACE: once_cell::sync::Lazy<bool> = once_cell::sync::Lazy::new(|| {
            std::env::var("GOSSIP_METRICS").map_or(false, |s| s == "trace")
        });
        let trace = *TRACE;
        write!(f, "Metrics:")?;
        let mut average_last_completion = std::time::Duration::default();
        let mut max_last_completion = std::time::Duration::default();
        let mut average_completion_frequency = std::time::Duration::default();
        let mut complete_rounds = 0;
        let mut min_complete_rounds = usize::MAX;
        for (key, info) in &self.agent_history {
            let completion_frequency: std::time::Duration =
                info.successes.iter().map(|i| i.elapsed()).sum();
            let completion_frequency = completion_frequency
                .checked_div(info.successes.len() as u32)
                .unwrap_or_default();
            let last_completion = info
                .successes
                .back()
                .map(|i| i.elapsed())
                .unwrap_or_default();
            average_last_completion += last_completion;
            max_last_completion = max_last_completion.max(last_completion);
            average_completion_frequency += completion_frequency;
            if !info.successes.is_empty() {
                complete_rounds += 1;
            }
            min_complete_rounds = min_complete_rounds.min(info.successes.len());
            if trace {
                write!(f, "\n\t{:?}:", key)?;
                write!(
                    f,
                    "\n\t\tErrors: {}, Last: {:?}",
                    info.errors.len(),
                    info.errors.back().map(|i| i.elapsed()).unwrap_or_default()
                )?;
                write!(
                    f,
                    "\n\t\tInitiates: {}, Last: {:?}",
                    info.initiates.len(),
                    info.initiates
                        .back()
                        .map(|i| i.elapsed())
                        .unwrap_or_default()
                )?;
                write!(
                    f,
                    "\n\t\tRemote Rounds: {}, Last: {:?}",
                    info.accepts.len(),
                    info.accepts.back().map(|i| i.elapsed()).unwrap_or_default()
                )?;
                write!(
                    f,
                    "\n\t\tComplete Rounds: {}, Last: {:?}, Average completion Frequency: {:?}",
                    info.successes.len(),
                    last_completion,
                    completion_frequency
                )?;
                write!(f, "\n\t\tCurrent Round: {:?}", info.current_round)?;
            }
        }
        write!(
            f,
            "\n\tNumber of remote nodes complete {} out of {}. Min per node: {}.",
            complete_rounds,
            self.agent_history.len(),
            min_complete_rounds
        )?;
        write!(
            f,
            "\n\tAverage time since last completion: {:?}",
            average_last_completion
                .checked_div(self.agent_history.len() as u32)
                .unwrap_or_default()
        )?;
        write!(
            f,
            "\n\tMax time since last completion: {:?}",
            max_last_completion
        )?;
        write!(
            f,
            "\n\tAverage completion frequency: {:?}",
            average_completion_frequency
                .checked_div(self.agent_history.len() as u32)
                .unwrap_or_default()
        )?;
        write!(f, "\n\tForce Initiate: {}", self.force_initiates)?;
        Ok(())
    }
}

/// Synchronization primitive around the Metrics struct.
#[derive(Clone)]
pub struct MetricsSync(Arc<parking_lot::RwLock<Metrics>>);

impl Default for MetricsSync {
    fn default() -> Self {
        Self(Arc::new(parking_lot::RwLock::new(Metrics::default())))
    }
}

impl std::fmt::Debug for MetricsSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.read().fmt(f)
    }
}

impl std::fmt::Display for MetricsSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.read().fmt(f)
    }
}

impl MetricsSync {
    /// Get a read lock for the metrics store.
    pub fn read(&self) -> parking_lot::RwLockReadGuard<Metrics> {
        match self.0.try_read_for(std::time::Duration::from_millis(100)) {
            Some(g) => g,
            // This won't block if a writer is waiting.
            // NOTE: This is a bit of a hack to work around a lock somewhere that is errant-ly
            // held over another call to lock. Really we should fix that error,
            // potentially by using a closure pattern here to ensure the lock cannot
            // be held beyond the access logic.
            None => self.0.read_recursive(),
        }
    }

    /// Get a write lock for the metrics store.
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<Metrics> {
        match self.0.try_write_for(std::time::Duration::from_secs(100)) {
            Some(g) => g,
            None => {
                eprintln!("Metrics lock likely deadlocked");
                self.0.write()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_avg() {
        let mut a1 = RunAvg::default();
        a1.push(100);
        a1.push(1);
        a1.push(1);
        a1.push(1);
        assert_eq!(25.75, *a1);

        let mut a2 = RunAvg::default();
        a2.push_n(100, 1);
        a2.push_n(1, 3);
        assert_eq!(25.75, *a2);

        let mut a3 = RunAvg::default();
        a3.push_n(100, 255);
        a3.push(1);
        assert_eq!(99.61176, *a3);

        let mut a4 = RunAvg::default();
        a4.push_n(100, 255);
        a4.push_n(1, 128);
        assert_eq!(50.30588, *a4);

        let mut a5 = RunAvg::default();
        a5.push_n(100, 255);
        a5.push_n(1, 255);
        assert_eq!(1.0, *a5);
    }
}
