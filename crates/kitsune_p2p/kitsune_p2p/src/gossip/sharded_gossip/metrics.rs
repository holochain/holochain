use Instant;

use super::*;

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
pub(super) struct Metrics {
    /// Map of remote nodes.
    map: HashMap<StateKey, NodeInfo>,
    // Number of times we need to force initiate
    // the next round.
    force_initiates: u8,
}

/// Outcome of a gossip round.
pub(super) enum RoundOutcome {
    Success(Instant),
    Error(Instant),
}

impl Metrics {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Record a gossip round has been initiated by us.
    pub(super) fn record_initiate(&mut self, key: StateKey) {
        let info = self.map.entry(key).or_default();
        record_instant(&mut info.initiates);
        info.current_round = true;
    }

    /// Record a remote gossip round has started.
    pub(super) fn record_remote_round(&mut self, key: StateKey) {
        let info = self.map.entry(key).or_default();
        record_instant(&mut info.remote_rounds);
        info.current_round = true;
    }

    /// Record a gossip round has completed successfully.
    pub(super) fn record_success(&mut self, key: StateKey) {
        let info = self.map.entry(key).or_default();
        record_instant(&mut info.complete_rounds);
        info.current_round = false;
        if info.is_initiate_round() {
            self.force_initiates = self.force_initiates.saturating_sub(1);
        }
    }

    /// Record a gossip round has finished with an error.
    pub(super) fn record_error(&mut self, key: StateKey) {
        let info = self.map.entry(key).or_default();
        record_instant(&mut info.errors);
        info.current_round = false;
    }

    /// Record that we should force initiate the next few rounds.
    pub(super) fn record_force_initiate(&mut self) {
        self.force_initiates = MAX_TRIGGERS;
    }

    /// Get the last successful round time.
    pub(super) fn last_success(&self, key: &StateKey) -> Option<&Instant> {
        self.map
            .get(key)
            .and_then(|info| info.complete_rounds.back())
    }

    /// Is this node currently in an active round?
    pub(super) fn is_current_round(&self, key: &StateKey) -> bool {
        dbg!(&self.map);
        dbg!(key.as_str());
        for k in self.map.keys() {
            dbg!(k.as_str());
        }
        self.map.get(key).map_or(false, |info| info.current_round)
    }

    /// What was the last outcome for this nodes gossip round?
    pub(super) fn last_outcome(&self, key: &StateKey) -> Option<RoundOutcome> {
        self.map.get(key).and_then(
            |info| match (info.errors.back(), info.complete_rounds.back()) {
                (Some(error), Some(success)) => {
                    if error > success {
                        Some(RoundOutcome::Error(*error))
                    } else {
                        Some(RoundOutcome::Success(*success))
                    }
                }
                (Some(error), None) => Some(RoundOutcome::Error(*error)),
                (None, Some(success)) => Some(RoundOutcome::Success(*success)),
                (None, None) => None,
            },
        )
    }

    /// Should we force initiate the next round?
    pub(super) fn forced_initiate(&self) -> bool {
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
        let mut average_last_completion = Duration::default();
        let mut max_last_completion = Duration::default();
        let mut average_completion_frequency = Duration::default();
        let mut complete_rounds = 0;
        let mut min_complete_rounds = usize::MAX;
        for (key, info) in &self.map {
            let completion_frequency: Duration =
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
