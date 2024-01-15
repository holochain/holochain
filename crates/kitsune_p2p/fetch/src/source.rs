use std::{default, collections::{HashMap, VecDeque}};

use kitsune_p2p_types::KAgent;
use linked_hash_map::LinkedHashMap;
use tokio::{time::{Duration, Instant}, sync::{Semaphore, SemaphorePermit}};
use crate::{TransferMethod, FetchBackoff, FetchKey};

/// A source to fetch from: either a node, or an agent on a node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FetchSource {
    /// An agent on a node
    Agent(KAgent),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SourceRecord {
    /// The source to fetch from
    pub(crate) source: FetchSource,

    pub(crate) transfer_method: TransferMethod,

    /// The last time we tried fetching from this source
    pub(crate) last_request: Option<Instant>,
}

impl SourceRecord {
    pub(crate) fn new(source: FetchSource, transfer_method: TransferMethod) -> Self {
        Self {
            source,
            transfer_method,
            last_request: None,
        }
    }
}

/// Fetch item within the fetch queue state.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Sources(VecDeque<FetchSource>);

impl Sources {
    pub(crate) fn new(queue: VecDeque<FetchSource>) -> Self {
        let mut this = Self(VecDeque::new());
        for source in queue {
            this.add(source);
        }

        this
    }

    pub(crate) fn next(&mut self, interval: Duration) -> Option<FetchSource> {
        match self.0.pop_front() {
            Some(next_source_key) => {
                self.0.push_back(next_source_key.clone());
                Some(next_source_key)
            },
            None => None,
        }
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
pub struct SourceState<'a> {
    /// The current state of the source
    current_state: SourceCurrentState,

    /// The times that outstanding requests were made to this source
    outstanding_requests: HashMap<FetchKey, OutstandingRequest<'a>>,
}

/// a thing
#[derive(Debug)]
pub struct OutstandingRequest<'a> {
    request_time: Instant,
    maybe_permit: Option<SemaphorePermit<'a>>,
}

impl <'a> SourceState<'a> {
    /// check
    pub fn should_use(&'a mut self, fetch_key: FetchKey, timeout: Duration) -> bool {
        let num_timed_out = self.drop_timed_out(timeout);

        match &mut self.current_state {
            SourceCurrentState::Available(current_num_timed_out) => {
                *current_num_timed_out += num_timed_out;


                self.outstanding_requests.insert(fetch_key, OutstandingRequest {
                    request_time: Instant::now(),
                    maybe_permit: None,
                });
                true
            }
            SourceCurrentState::Backoff(backoff) => {
                match backoff.should_use_source() {
                    None => false,
                    permit => {
                        self.outstanding_requests.insert(fetch_key, OutstandingRequest {
                            request_time: Instant::now(),
                            maybe_permit: permit,
                        });
                        true
                    }
                }
            }
        }
    }

    /// check
    pub fn should_remove(&self) -> bool {
        match self.current_state {
            SourceCurrentState::Available(_) => false,
            SourceCurrentState::Backoff(ref backoff) => backoff.is_expired(),
        }
    }

    /// check
    pub fn response_received(&mut self, fetch_key: &FetchKey) {
        self.outstanding_requests.remove(fetch_key);
    }

    /// check
    fn drop_timed_out(&mut self, timeout: Duration) -> usize {
        let current_size = self.outstanding_requests.len();
        self.outstanding_requests.retain(|_, r| r.request_time.elapsed() < timeout);
        current_size - self.outstanding_requests.len()
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
    probe_permit: Semaphore,
}

impl FetchSourceBackoff {
    fn should_use_source(&mut self) -> Option<SemaphorePermit<'_>> {
        if self.backoff.is_ready() {
            match self.probe_permit.try_acquire() {
                Ok(permit) => Some(permit),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    fn is_expired(&self) -> bool {
        self.backoff.is_expired()
    }
}
