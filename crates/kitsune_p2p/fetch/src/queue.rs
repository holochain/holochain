//! The Fetch Queue: a structure to store ops-to-be-fetched.
//!
//! When we encounter an op hash that we have no record of, we store it as an item
//! at the end of the FetchQueue. The items of the queue contain not only the op hash,
//! but also the source(s) to fetch it from, and other data including the last time
//! a fetch was attempted.
//!
//! The consumer of the queue can read items whose last_fetch time is older than some interval
//! from the current moment. The items thus returned are not guaranteed to be returned in
//! order of last_fetch time, but they are guaranteed to be at least as old as the specified
//! interval.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use kitsune_p2p_types::{tx2::tx2_utils::Share, KAgent, KSpace /*, Tx2Cert*/};
use linked_hash_map::{Entry, LinkedHashMap};

use crate::{FetchContext, FetchKey, FetchOptions, FetchQueuePush, RoughInt};

mod queue_reader;
pub use queue_reader::*;

/// Max number of queue items to check on each `next()` poll
const NUM_ITEMS_PER_POLL: usize = 100;

/// A FetchQueue tracks a set of [`FetchKey`]s (op hashes or regions) to be fetched,
/// each of which can have multiple sources associated with it.
///
/// When adding the same key twice, the sources are merged by appending the newest
/// source to the front of the list of sources, and the contexts are merged by the
/// method defined in [`FetchQueueConfig`].
///
/// The queue items can be accessed only through its Iterator implementation.
/// Each item contains a FetchKey and one Source agent from which to fetch it.
/// Each time an item is obtained in this way, it is moved to the end of the list.
/// It is important to use the iterator lazily, and only take what is needed.
/// Accessing any item through iteration implies that a fetch was attempted.
#[derive(Clone)]
pub struct FetchQueue {
    config: FetchConfig,
    state: Share<State>,
}

impl std::fmt::Debug for FetchQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.state
            .share_ref(|state| Ok(f.debug_struct("FetchQueue").field("state", state).finish()))
            .unwrap()
    }
}

/// Alias
pub type FetchConfig = Arc<dyn FetchQueueConfig>;

/// Host-defined details about how the fetch queue should function
pub trait FetchQueueConfig: 'static + Send + Sync {
    /// How often we should attempt to fetch items by source.
    fn fetch_retry_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5 * 60)
    }

    /// When a fetch key is added twice, this determines how the two different contexts
    /// get reconciled.
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32;
}

/// The actual inner state of the FetchQueue, from which items can be obtained
#[derive(Default, Debug)]
pub struct State {
    /// Items ready to be fetched
    queue: LinkedHashMap<FetchKey, FetchQueueItem>,
}

/// A mutable iterator over the FetchQueue State
pub struct StateIter<'a> {
    state: &'a mut State,
    interval: Duration,
}

/// Fetch item within the fetch queue state.
#[derive(Debug, PartialEq, Eq)]
struct Sources(Vec<SourceRecord>);

/// An item in the queue, corresponding to a single op or region to fetch
#[derive(Debug, PartialEq, Eq)]
pub struct FetchQueueItem {
    /// Known sources from whom we can fetch this item.
    /// Sources will always be tried in order.
    sources: Sources,
    /// The space to retrieve this op from
    space: KSpace,
    /// Approximate size of the item. If set, the item will be counted towards overall progress.
    size: Option<RoughInt>,
    /// Options specified for this fetch job
    options: Option<FetchOptions>,
    /// Opaque user data specified by the host
    pub context: Option<FetchContext>,
}

#[derive(Debug, PartialEq, Eq)]
struct SourceRecord {
    source: FetchSource,
    last_fetch: Option<Instant>,
}

/// A source to fetch from: either a node, or an agent on a node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchSource {
    /// An agent on a node
    Agent(KAgent),
    // /// A node, without agent specified
    // Node(Tx2Cert),
}

// TODO: move this to host, but for now, for convenience, we just use this one config
// for every queue
struct FetchQueueConfigBitwiseOr;

impl FetchQueueConfig for FetchQueueConfigBitwiseOr {
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
        a | b
    }
}

impl FetchQueue {
    /// Constructor
    pub fn new(config: FetchConfig) -> Self {
        Self {
            config,
            state: Share::new(State::default()),
        }
    }

    /// Constructor, using only the "hardcoded" config (TODO: remove)
    pub fn new_bitwise_or() -> Self {
        Self {
            config: Arc::new(FetchQueueConfigBitwiseOr),
            state: Share::new(State::default()),
        }
    }

    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    pub fn push(&self, args: FetchQueuePush) {
        self.state
            .share_mut(|s, _| {
                tracing::debug!(
                    "FetchQueue (size = {}) item added: {:?}",
                    s.queue.len() + 1,
                    args
                );
                s.push(&*self.config, args);
                Ok(())
            })
            .expect("no error");
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    pub fn remove(&self, key: &FetchKey) -> Option<FetchQueueItem> {
        self.state
            .share_mut(|s, _| {
                let removed = s.remove(key);
                tracing::debug!(
                    "FetchQueue (size = {}) item removed: key={:?} val={:?}",
                    s.queue.len(),
                    key,
                    removed
                );
                Ok(removed)
            })
            .expect("no error")
    }

    /// Get a list of the next items that should be fetched.
    pub fn get_items_to_fetch(&self) -> Vec<(FetchKey, KSpace, FetchSource)> {
        let interval = self.config.fetch_retry_interval();
        self.state
            .share_mut(|s, _| {
                let mut out = Vec::new();

                for (key, space, source) in s.iter_mut(interval) {
                    out.push((key, space, source));
                }

                Ok(out)
            })
            .expect("no error")
    }
}

impl State {
    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    pub fn push(&mut self, config: &dyn FetchQueueConfig, args: FetchQueuePush) {
        let FetchQueuePush {
            key,
            author,
            options,
            context,
            space,
            source,
            size,
        } = args;

        match self.queue.entry(key) {
            Entry::Vacant(e) => {
                let sources = if let Some(author) = author {
                    Sources(vec![SourceRecord::new(source), SourceRecord::agent(author)])
                } else {
                    Sources(vec![SourceRecord::new(source)])
                };
                let item = FetchQueueItem {
                    sources,
                    space,
                    size,
                    options,
                    context,
                };
                e.insert(item);
            }
            Entry::Occupied(mut e) => {
                let v = e.get_mut();
                v.sources.0.insert(0, SourceRecord::new(source));
                v.options = options;
                v.context = match (v.context.take(), context) {
                    (Some(a), Some(b)) => Some(config.merge_fetch_contexts(*a, *b).into()),
                    (a, b) => a.and(b),
                }
            }
        }
    }

    /// Access queue items through mutable iteration. Items accessed will be moved
    /// to the end of the queue.
    ///
    /// Only items whose `last_fetch` is more than `interval` ago will be returned.
    pub fn iter_mut(&mut self, interval: Duration) -> StateIter {
        StateIter {
            state: self,
            interval,
        }
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    pub fn remove(&mut self, key: &FetchKey) -> Option<FetchQueueItem> {
        self.queue.remove(key)
    }
}

impl<'a> Iterator for StateIter<'a> {
    type Item = (FetchKey, KSpace, FetchSource);

    fn next(&mut self) -> Option<Self::Item> {
        let keys: Vec<_> = self
            .state
            .queue
            .keys()
            .take(NUM_ITEMS_PER_POLL)
            .cloned()
            .collect();
        for key in keys {
            let item = self.state.queue.get_refresh(&key)?;
            if let Some(source) = item.sources.next(self.interval) {
                let space = item.space.clone();
                return Some((key, space, source));
            }
        }
        None
    }
}

impl SourceRecord {
    fn new(source: FetchSource) -> Self {
        Self {
            source,
            last_fetch: None,
        }
    }

    fn agent(agent: KAgent) -> Self {
        Self {
            source: FetchSource::Agent(agent),
            last_fetch: None,
        }
    }
}

impl Sources {
    fn next(&mut self, interval: Duration) -> Option<FetchSource> {
        if let Some((i, agent)) = self
            .0
            .iter()
            .enumerate()
            .find(|(_, s)| s.last_fetch.map(|t| t.elapsed() > interval).unwrap_or(true))
            .map(|(i, s)| (i, s.source.clone()))
        {
            self.0[i].last_fetch = Some(Instant::now());
            self.0.rotate_left(i + 1);
            Some(agent)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;
    use std::{sync::Arc, time::Duration};

    use kitsune_p2p_types::bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash, KitsuneSpace};

    use super::*;

    pub(super) struct Config;

    impl FetchQueueConfig for Config {
        fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
            (a + b).min(1)
        }
    }

    pub(super) fn key_op(n: u8) -> FetchKey {
        FetchKey::Op(Arc::new(KitsuneOpHash::new(vec![n; 36])))
    }

    pub(super) fn req(n: u8, context: Option<FetchContext>, source: FetchSource) -> FetchQueuePush {
        FetchQueuePush {
            key: key_op(n),
            author: None,
            context,
            options: Default::default(),
            space: space(0),
            source,
            size: None,
        }
    }

    pub(super) fn item(sources: Vec<FetchSource>, context: Option<FetchContext>) -> FetchQueueItem {
        FetchQueueItem {
            sources: Sources(sources.into_iter().map(SourceRecord::new).collect()),
            space: Arc::new(KitsuneSpace::new(vec![0; 36])),
            options: Default::default(),
            context,
            size: None,
        }
    }

    pub(super) fn space(i: u8) -> KSpace {
        Arc::new(KitsuneSpace::new(vec![i; 36]))
    }

    pub(super) fn source(i: u8) -> FetchSource {
        FetchSource::Agent(Arc::new(KitsuneAgent::new(vec![i; 36])))
    }

    pub(super) fn sources(ix: impl IntoIterator<Item = u8>) -> Vec<FetchSource> {
        ix.into_iter().map(source).collect()
    }

    pub(super) fn ctx(c: u32) -> Option<FetchContext> {
        Some(c.into())
    }

    #[test]
    fn source_rotation() {
        let mut ss = Sources(vec![
            SourceRecord {
                source: source(1),
                last_fetch: Some(Instant::now()),
            },
            SourceRecord {
                source: source(2),
                last_fetch: None,
            },
        ]);

        assert_eq!(ss.next(Duration::from_secs(10)), Some(source(2)));
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_nanos(1)), Some(source(1)));
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_nanos(1)), Some(source(2)));
        assert_eq!(ss.next(Duration::from_nanos(1)), Some(source(1)));
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_secs(10)), None);
    }

    #[test]
    fn queue_push() {
        let mut q = State::default();
        let c = Config;

        // note: new sources get added to the front of the list
        q.push(&c, req(1, ctx(1), source(1)));
        q.push(&c, req(1, ctx(0), source(0)));

        q.push(&c, req(2, ctx(0), source(0)));

        let expected_ready = [
            (key_op(1), item(sources(0..=1), ctx(1))),
            (key_op(2), item(sources([0]), ctx(0))),
        ]
        .into_iter()
        .collect();

        assert_eq!(q.queue, expected_ready);
    }

    #[test]
    fn queue_next() {
        let mut q = {
            let mut queue = [
                (key_op(1), item(sources(0..=2), ctx(1))),
                (key_op(2), item(sources(1..=3), ctx(1))),
                (key_op(3), item(sources(2..=4), ctx(1))),
            ];
            // Set the last_fetch time of one of the sources to something a bit earlier,
            // so it won't show up in next() right away
            queue[1].1.sources.0[1].last_fetch = Some(Instant::now() - Duration::from_secs(3));

            let queue = queue.into_iter().collect();
            State { queue }
        };
        assert_eq!(q.iter_mut(Duration::from_secs(10)).count(), 8);

        // The next (and only) item will be the one with the timestamp explicitly set
        assert_eq!(
            q.iter_mut(Duration::from_secs(1)).collect::<Vec<_>>(),
            vec![(key_op(2), space(0), source(2))]
        );

        // When traversing the entire queue again, the "special" item is still the last one.
        let items: Vec<_> = q.iter_mut(Duration::from_millis(0)).take(9).collect();
        assert_eq!(items[8], (key_op(2), space(0), source(2)));

        // We traversed all items in the last second, so this returns None
        assert_eq!(q.iter_mut(Duration::from_secs(1)).next(), None);
    }
}
