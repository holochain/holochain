use std::{default, collections::VecDeque};
use kitsune_p2p_types::KAgent;
use tokio::time::Duration;
use crate::FetchBackoff;

/// A source to fetch from: either a node, or an agent on a node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FetchSource {
    /// An agent on a node
    Agent(KAgent),
}

// TODO this wrapper needs work, use indexmap and clean up the custom implementation
/// Fetch item within the fetch queue state.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Sources(pub VecDeque<FetchSource>);

impl Sources {
    pub(crate) fn new(queue: VecDeque<FetchSource>) -> Self {
        let mut this = Self(VecDeque::new());
        for source in queue {
            this.add(source);
        }

        this
    }

    pub(crate) fn next<T>(&mut self, mut state_filter: T) -> Option<FetchSource> where T: FnMut(&FetchSource) -> bool {
        for _ in 0..self.0.len() {
            let source = self.0.pop_front().unwrap();
            self.0.push_back(source.clone());

            if state_filter(&source) {
                return Some(source);
            }
        }

        None
    }

    pub(crate) fn add(&mut self, source: FetchSource) {
        if self.0.contains(&source) {
            return;
        }

        self.0.push_back(source);
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

/// The state of a source
#[derive(Debug, Default)]
pub struct SourceState {
    /// The current state of the source
    current_state: SourceCurrentState,

    /// The number of requests to this source that have timed out
    timed_out_count: usize,
}

impl SourceState {
    /// check
    pub fn should_use(&mut self) -> bool {
        match &mut self.current_state {
            SourceCurrentState::Available(_) => {
                true
            }
            SourceCurrentState::Backoff(backoff) => {
                if backoff.should_use_source() {
                    true
                } else {
                    false
                }
            }
        }
    }

    /// check
    pub fn check(&mut self) -> bool {
        match &self.current_state {
            SourceCurrentState::Available(num_timed_out) => {
                if *num_timed_out > 100 {
                    self.current_state = SourceCurrentState::Backoff(FetchSourceBackoff {
                        backoff: FetchBackoff::new(Duration::from_secs(1)),
                        probe_limit: 10,
                    });
                }

                true
            },
            SourceCurrentState::Backoff(ref backoff) => backoff.is_expired(),
        }
    }

    /// check
    pub fn record_timeout(&mut self) {
        self.timed_out_count += 1;
    }

    /// record response
    pub fn record_response(&mut self) {
        match &self.current_state {
            SourceCurrentState::Backoff(_) => {
                self.current_state = SourceCurrentState::Available(0);
            }
            SourceCurrentState::Available(_) => (),
        }
    }
}

/// The state of a source
#[derive(Debug)]
pub enum SourceCurrentState {
    /// As far as we know, this source is available and responding
    Available(usize),

    /// The source has been unavailable and we're waiting for a backoff period to expire before trying again.
    Backoff(FetchSourceBackoff),
}

impl default::Default for SourceCurrentState {
    fn default() -> Self {
        Self::Available(0)
    }
}

/// a struct
#[derive(Debug)]
pub struct FetchSourceBackoff {
    backoff: FetchBackoff,
    probe_limit: u32,
}

impl FetchSourceBackoff {
    fn should_use_source(&mut self) -> bool {
        if self.backoff.is_ready() {
            self.probe_limit = 10; // Grant more probes for this retry
            true
        } else {
            if self.probe_limit > 0 {
                self.probe_limit -=1;
                true
            } else {
                // Probes exhausted, wait for the backoff to expire and grant more probes
                false
            }
        }
    }

    fn is_expired(&self) -> bool {
        self.backoff.is_expired()
    }
}
