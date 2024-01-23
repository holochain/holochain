use indexmap::IndexSet;
use kitsune_p2p_types::KAgent;
use std::{default, ops::Deref};

use crate::{backoff::FetchBackoff, FetchConfig};

/// The number of times to probe a source between backoff attempts. This needs to be enough to reasonably allow
/// a source which might be slow to respond to one or two requests to respond to at least one of the probes but
/// not so high that it wastes time for this node trying to talk to a source that is not responding.
const NUM_PROBE_ATTEMPTS: u32 = 10;

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
    pub(crate) fn new(queue: impl IntoIterator<Item = FetchSource>) -> Self {
        Self {
            inner: queue.into_iter().collect(),
            index: 0,
        }
    }

    pub(crate) fn next(
        &mut self,
        mut state_filter: impl FnMut(&FetchSource) -> bool,
    ) -> Option<FetchSource> {
        for _ in 0..self.inner.len() {
            let fetch_index = self.index;
            self.index = (self.index + 1) % self.inner.len();

            if let Some(source) = self.inner.get_index(fetch_index) {
                if state_filter(source) {
                    return Some(source.clone());
                }
            }
        }

        None
    }

    pub(crate) fn add(&mut self, source: FetchSource) {
        self.inner.insert(source);
    }

    pub(crate) fn retain(&mut self, filter: impl Fn(&FetchSource) -> bool) {
        self.inner.retain(filter);

        // Ensure the index is still valid
        if !self.inner.is_empty() {
            self.index %= self.inner.len();
        }
    }
}

/// The state of a source
#[derive(Debug, Default)]
pub(crate) struct SourceState {
    /// The current state of the source
    current_state: SourceCurrentState,
}

impl SourceState {
    /// Check whether this source should be used. If this source is currently considered available then it will always be usable.
    /// Otherwise, when the source is in a backoff state, it will only be usable if the backoff is ready. The backoff will be ready
    /// a fixed number of times to probe the source before going back into a backoff state. If any fetches from the
    /// probe attempts succeed then the source will be considered available again.
    pub fn should_use(&mut self) -> bool {
        match &mut self.current_state {
            SourceCurrentState::Available(_) => true,
            SourceCurrentState::Backoff(backoff) => backoff.is_ready(),
        }
    }

    /// Check the state of this source. If the source has had too many timeouts then it is still considered valid but it will be put into a backoff state.
    /// If the source is in a backoff state and the backoff has expired, then the check fails and this source should be dropped.
    pub fn is_valid(&mut self, config: FetchConfig) -> bool {
        match &self.current_state {
            SourceCurrentState::Available(num_timed_out) => {
                if *num_timed_out > config.source_unavailable_timeout_threshold() {
                    self.current_state = SourceCurrentState::Backoff(FetchSourceBackoff::new(
                        FetchBackoff::new(config.source_retry_delay()),
                        NUM_PROBE_ATTEMPTS,
                    ));
                }

                true
            }
            SourceCurrentState::Backoff(ref backoff) => !backoff.is_expired(),
        }
    }

    /// Notify the state that a request to this source has timed out.
    pub fn record_timeout(&mut self) {
        if let SourceCurrentState::Available(num_timeouts) = &mut self.current_state {
            *num_timeouts += 1;
        }
    }

    /// Notify the state that a request to this source has succeeded.
    /// If the source is in a backoff state then it will be considered available again.
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
    /// As far as we know, this source is available and responding.
    ///
    /// The inner value tracks the number of requests to this source that have timed out.
    /// Note that these failures do not age out, so if a source is unreliable it will get put on a timeout
    /// briefly after it fails to respond too many times. This isn't a bad thing if the source is
    /// not responding because it is overwhelmed.
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
    probes: u32,
}

impl FetchSourceBackoff {
    fn new(backoff: FetchBackoff, probe_limit: u32) -> Self {
        Self {
            backoff,
            probe_limit,
            probes: 0,
        }
    }

    fn is_ready(&mut self) -> bool {
        if self.backoff.is_ready() {
            self.probes = self.probe_limit - 1; // Grant more probes for this retry
            true
        } else if self.probes > 0 {
            self.probes -= 1;
            true
        } else {
            // Probes exhausted, wait for the backoff to expire and grant more probes
            false
        }
    }

    fn is_expired(&self) -> bool {
        self.backoff.is_expired()
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    #[allow(warnings)]
    use super::{SourceState, Sources, NUM_PROBE_ATTEMPTS};
    use crate::{
        backoff::{FetchBackoff, BACKOFF_RETRY_COUNT},
        source::FetchSourceBackoff,
        test_utils::*,
        FetchPoolConfig,
    };

    #[test]
    fn single_source() {
        let mut sources = Sources::new([test_source(1)]);

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

    #[tokio::test]
    async fn fetch_source_backoff() {
        let mut backoff = FetchSourceBackoff::new(FetchBackoff::new(Duration::from_millis(10)), 3);
        assert!(!backoff.is_ready());

        let mut num_tries = 0;
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if backoff.is_ready() {
                    num_tries += 1;
                } else if backoff.is_expired() {
                    break;
                }
            }
        })
        .await
        .unwrap();

        // Number of probes per ready (3), multiplied by the number of attempts that FetchBackoff allows (BACKOFF_RETRY_COUNT)
        assert_eq!(3 * BACKOFF_RETRY_COUNT, num_tries);
    }

    #[test]
    fn happy_path_source_state() {
        let mut source_state: SourceState = Default::default();
        let config = Arc::new(TestFetchConfig(1, 1));

        for i in 0..500 {
            assert!(source_state.should_use());
            source_state.record_response();

            if i % 100 == 0 {
                assert!(source_state.is_valid(config.clone()));
            }
        }

        assert!(source_state.should_use());
    }

    #[tokio::test(start_paused = true)]
    async fn source_state_single_backoff_then_recover() {
        let mut source_state: SourceState = Default::default();
        let config = Arc::new(TestFetchConfig(1, 1));

        assert!(source_state.should_use());

        // Exhaust the retries
        for _ in 0..=config.source_unavailable_timeout_threshold() {
            // The source should continue being used even with timeouts. It's only when we hit the limit that it shouldn't.
            assert!(source_state.should_use());

            // The check should keep passing
            source_state.is_valid(config.clone());

            // Record another timeout
            source_state.record_timeout();
        }

        // The source is still ready because it hasn't been checked
        assert!(source_state.should_use());

        // Now it goes into a backoff state
        source_state.is_valid(config.clone());
        assert!(!source_state.should_use());

        tokio::time::advance(Duration::from_secs(2)).await;

        // Now the backoff should go into a ready state and permit a number of probes
        for _ in 0..NUM_PROBE_ATTEMPTS {
            assert!(source_state.should_use());
        }

        // The probes have all been used and the backoff should be waiting again
        assert!(!source_state.should_use());

        // Now get a single successful response
        source_state.record_response();

        // Go back to a ready state
        assert!(source_state.should_use());
    }

    #[tokio::test(start_paused = true)]
    async fn source_state_backoff_to_expiry() {
        let mut source_state: SourceState = Default::default();
        let config = Arc::new(TestFetchConfig(1, 1));

        assert!(source_state.should_use());

        // Exhaust the retries
        for _ in 0..=config.source_unavailable_timeout_threshold() {
            source_state.record_timeout();
        }
        source_state.is_valid(config.clone());
        assert!(!source_state.should_use());

        for _ in 0..BACKOFF_RETRY_COUNT {
            // Just move by a lot to guarantee that we're back in a ready state, without hitting probes because that's irrelevant for this test
            tokio::time::advance(100 * config.source_retry_delay()).await;

            assert!(source_state.should_use());
        }

        // The source state is now dead and should be removed.
        assert!(!source_state.is_valid(config.clone()));
    }
}
