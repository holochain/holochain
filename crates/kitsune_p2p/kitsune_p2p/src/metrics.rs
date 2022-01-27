//! metrics tracked by kitsune_p2p spaces

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

use tokio::time::Instant;

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
/// Information about a remote node.
struct NodeInfo {
    /// Sucessful and unsuccessful messages from the remote
    /// can be combined to estimate a "reachability quotient"
    /// between 1 (or 0 if empty) and 100. Errors are weighted
    /// heavier because we retry less frequently.
    reachability_quotient: RunAvg,
    /// Running average for latency microseconds for any direct
    /// request/response calls to remote agent.
    latency_micros: RunAvg,
    /// Times we recorded errors for this node.
    errors: VecDeque<Instant>,
    /// Times we recorded initiates to this node.
    initiates: VecDeque<Instant>,
    /// Times we recorded remote rounds from this node.
    remote_rounds: VecDeque<Instant>,
    /// Times we recorded complete rounds for this node.
    complete_rounds: VecDeque<Instant>,
    /// Is this node currently in an active round?
    current_round: bool,
}

#[derive(Debug, Default)]
/// Metrics tracking for remote nodes to help
/// choose which remote node to initiate the next round with.
pub struct Metrics {
    /// Map of remote agents.
    map: HashMap<Arc<KitsuneAgent>, NodeInfo>,

    /// Aggregate Extrapolated Dht Coverage
    agg_extrap_cov: RunAvg,

    // Number of times we need to force initiate
    // the next round.
    force_initiates: u8,
}

/// Outcome of a gossip round.
#[derive(PartialOrd, Ord, PartialEq, Eq)]
pub enum RoundOutcome {
    /// Success outcome
    Success(Instant),
    /// Error outcome
    Error(Instant),
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

        for (agent, node) in self.map.iter() {
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
            .map
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
                .map
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
            let info = self
                .map
                .entry(agent_info.into().agent().clone())
                .or_default();
            info.latency_micros.push(micros);
        }
    }

    /// Record a gossip round has been initiated by us.
    pub fn record_initiate<'a, T, I>(&mut self, remote_agent_list: I)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        for agent_info in remote_agent_list {
            let info = self
                .map
                .entry(agent_info.into().agent().clone())
                .or_default();
            record_instant(&mut info.initiates);
            info.current_round = true;
        }
    }

    /// Record a remote gossip round has started.
    pub fn record_remote_round<'a, T, I>(&mut self, remote_agent_list: I)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        for agent_info in remote_agent_list {
            let info = self
                .map
                .entry(agent_info.into().agent().clone())
                .or_default();
            record_instant(&mut info.remote_rounds);
            info.current_round = true;
        }
    }

    /// Record a gossip round has completed successfully.
    pub fn record_success<'a, T, I>(&mut self, remote_agent_list: I)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        let mut should_dec_force_initiates = false;

        for agent_info in remote_agent_list {
            let info = self
                .map
                .entry(agent_info.into().agent().clone())
                .or_default();
            info.reachability_quotient.push(100);
            record_instant(&mut info.complete_rounds);
            info.current_round = false;
            if info.is_initiate_round() {
                should_dec_force_initiates = true;
            }
        }

        if should_dec_force_initiates {
            self.force_initiates = self.force_initiates.saturating_sub(1);
        }
    }

    /// Record a gossip round has finished with an error.
    pub fn record_error<'a, T, I>(&mut self, remote_agent_list: I)
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        for agent_info in remote_agent_list {
            let info = self
                .map
                .entry(agent_info.into().agent().clone())
                .or_default();
            info.reachability_quotient.push_n(1, 5);
            record_instant(&mut info.errors);
            info.current_round = false;
        }
    }

    /// Record that we should force initiate the next few rounds.
    pub fn record_force_initiate(&mut self) {
        self.force_initiates = MAX_TRIGGERS;
    }

    /// Get the last successful round time.
    pub fn last_success<'a, T, I>(&self, remote_agent_list: I) -> Option<&Instant>
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        remote_agent_list
            .into_iter()
            .filter_map(|agent_info| self.map.get(agent_info.into().agent()))
            .map(|info| info.complete_rounds.back())
            .flatten()
            .min()
    }

    /// Is this node currently in an active round?
    pub fn is_current_round<'a, T, I>(&self, remote_agent_list: I) -> bool
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        remote_agent_list
            .into_iter()
            .filter_map(|agent_info| self.map.get(agent_info.into().agent()))
            .map(|info| info.current_round)
            .any(|x| x)
    }

    /// What was the last outcome for this node's gossip round?
    pub fn last_outcome<'a, T, I>(&self, remote_agent_list: I) -> Option<RoundOutcome>
    where
        T: Into<AgentLike<'a>>,
        I: IntoIterator<Item = T>,
    {
        remote_agent_list
            .into_iter()
            .filter_map(|agent_info| self.map.get(agent_info.into().agent()))
            .map(|info| {
                [
                    info.errors.back().map(|x| RoundOutcome::Error(*x)),
                    info.complete_rounds
                        .back()
                        .map(|x| RoundOutcome::Success(*x)),
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
            .filter_map(|agent_info| self.map.get(agent_info.into().agent()))
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
            .filter_map(|agent_info| self.map.get(agent_info.into().agent()))
            .map(|info| *info.latency_micros)
            .fold((0.0, 0.0), |acc, x| (acc.0 + x, acc.1 + 1.0));
        if cnt <= 0.0 {
            0.0
        } else {
            sum / cnt
        }
    }
}

impl NodeInfo {
    /// Was the last round for this node initiated by us?
    fn is_initiate_round(&self) -> bool {
        match (self.remote_rounds.back(), self.initiates.back()) {
            (None, None) | (Some(_), None) => false,
            (None, Some(_)) => true,
            (Some(remote), Some(initiate)) => initiate > remote,
        }
    }
}

fn record_instant(buffer: &mut VecDeque<Instant>) {
    if buffer.len() > MAX_HISTORY {
        buffer.pop_front();
    }
    buffer.push_back(Instant::now());
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
        for (key, info) in &self.map {
            let completion_frequency: std::time::Duration =
                info.complete_rounds.iter().map(|i| i.elapsed()).sum();
            let completion_frequency = completion_frequency
                .checked_div(info.complete_rounds.len() as u32)
                .unwrap_or_default();
            let last_completion = info
                .complete_rounds
                .back()
                .map(|i| i.elapsed())
                .unwrap_or_default();
            average_last_completion += last_completion;
            max_last_completion = max_last_completion.max(last_completion);
            average_completion_frequency += completion_frequency;
            if !info.complete_rounds.is_empty() {
                complete_rounds += 1;
            }
            min_complete_rounds = min_complete_rounds.min(info.complete_rounds.len());
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
                    info.remote_rounds.len(),
                    info.remote_rounds
                        .back()
                        .map(|i| i.elapsed())
                        .unwrap_or_default()
                )?;
                write!(
                    f,
                    "\n\t\tComplete Rounds: {}, Last: {:?}, Average completion Frequency: {:?}",
                    info.complete_rounds.len(),
                    last_completion,
                    completion_frequency
                )?;
                write!(f, "\n\t\tCurrent Round: {}", info.current_round)?;
            }
        }
        write!(
            f,
            "\n\tNumber of remote nodes complete {} out of {}. Min per node: {}.",
            complete_rounds,
            self.map.len(),
            min_complete_rounds
        )?;
        write!(
            f,
            "\n\tAverage time since last completion: {:?}",
            average_last_completion
                .checked_div(self.map.len() as u32)
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
                .checked_div(self.map.len() as u32)
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
