use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

use tokio::time::Instant;

use crate::types::*;
use kitsune_p2p_types::agent_info::AgentInfoSigned;

/// The maximum number of different nodes that will be
/// gossiped with if gossip is triggered.
const MAX_TRIGGERS: u8 = 2;

/// Maximum amount of history we will track
/// per remote node.
const MAX_HISTORY: usize = 10;

#[derive(Debug, Clone, Default)]
/// Information about a remote node.
struct NodeInfo {
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

    // Number of times we need to force initiate
    // the next round.
    force_initiates: u8,
}

/// Outcome of a gossip round.
#[derive(PartialOrd, Ord, PartialEq, Eq)]
pub enum RoundOutcome {
    Success(Instant),
    Error(Instant),
}

pub enum AgentLike<'lt> {
    Info(&'lt AgentInfoSigned),
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
    pub fn agent(&self) -> &Arc<KitsuneAgent> {
        match self {
            Self::Info(i) => &i.agent,
            Self::PubKey(pk) => pk,
        }
    }
}

impl Metrics {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
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
