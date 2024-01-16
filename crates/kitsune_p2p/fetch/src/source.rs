use std::{default, ops::Deref};
use indexmap::IndexSet;
use kitsune_p2p_types::KAgent;
use tokio::time::Duration;

use crate::backoff::FetchBackoff;

/// A source to fetch from: either a node, or an agent on a node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FetchSource {
    /// An agent on a node
    Agent(KAgent),
}

/// Fetch item within the fetch queue state.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Sources {
    inner: IndexSet<FetchSource>,
    index: usize,
}

impl Deref for Sources {
    type Target = IndexSet<FetchSource>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Sources {
    pub(crate) fn new(queue: impl IntoIterator<Item=FetchSource>) -> Self {
        Self {
            inner: queue.into_iter().collect(),
            index: 0,
        }
    }

    pub(crate) fn next(&mut self, mut state_filter: impl FnMut(&FetchSource) -> bool) -> Option<FetchSource> {
        for _ in 0..self.inner.len() {
            let fetch_index = self.index;
            self.index = (self.index + 1) % self.inner.len();

            match self.inner.get_index(fetch_index) {
                Some(source) => {
                    if state_filter(source) {
                        return Some(source.clone());
                    }
                }
                None => (),
            }
        }

        None
    }

    pub(crate) fn add(&mut self, source: FetchSource) {
        self.inner.insert(source);
    }

    pub(crate) fn retain(&mut self, filter: impl Fn(&FetchSource) -> bool) {
        self.inner.retain(filter);
    }
}

/// The state of a source
#[derive(Debug, Default)]
pub(crate) struct SourceState {
    /// The current state of the source
    current_state: SourceCurrentState,

    /// The number of requests to this source that have timed out.
    /// 
    /// Note that these failures do not age out, so if a source is unreliable it will get put on a timeout
    /// briefly after it fails to respond too many times. This isn't a bad thing if the source is
    /// not responding because it is overwhelmed.
    timeout_count: usize,
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
        self.timeout_count += 1;
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

#[cfg(test)]
mod tests {
    use super::Sources;
    use crate::test_utils::*;

    #[test]
    fn single_source() {
        let mut sources = Sources::new(
            [
                test_source(1),
            ],
        );

        // The first source is returned
        assert_eq!(Some(test_source(1)), sources.next(|_| true));
        // The first source is returned a second time with no delay
        assert_eq!(Some(test_source(1)), sources.next(|_| true));
        // The first source can be filtered out
        assert_eq!(None, sources.next(|_| false));
    }

    #[test]
    fn source_rotation() {
        let mut sources = Sources::new([]);
        sources.add(test_source(1));
        sources.add(test_source(2));

        assert_eq!(Some(test_source(1)), sources.next(|_| true));
        assert_eq!(Some(test_source(2)), sources.next(|_| true));
        assert_eq!(Some(test_source(1)), sources.next(|_| true));

        sources.add(test_source(3));
        assert_eq!(Some(test_source(2)), sources.next(|_| true));
        assert_eq!(Some(test_source(3)), sources.next(|_| true));
        assert_eq!(Some(test_source(1)), sources.next(|_| true));
    }
}
