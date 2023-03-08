//! The Fetch Pool: a structure to store ops-to-be-fetched.
//!
//! When we encounter an op hash that we have no record of, we store it as an item
//! at the end of the FetchPool. The items of the queue contain not only the op hash,
//! but also the source(s) to fetch it from, and other data including the last time
//! a fetch was attempted.
//!
//! The consumer of the queue can read items whose last_fetch time is older than some interval
//! from the current moment. The items thus returned are not guaranteed to be returned in
//! order of last_fetch time, but they are guaranteed to be at least as old as the specified
//! interval.

use std::sync::Arc;
use tokio::time::{Duration, Instant};

use kitsune_p2p_types::{tx2::tx2_utils::ShareOpen, KAgent, KSpace /*, Tx2Cert*/};
use linked_hash_map::{Entry, LinkedHashMap};

use crate::{FetchContext, FetchKey, FetchPoolPush, RoughInt};

mod pool_reader;
pub use pool_reader::*;

/// Max number of queue items to check on each `next()` poll
const NUM_ITEMS_PER_POLL: usize = 100;

/// A FetchPool tracks a set of [`FetchKey`]s (op hashes or regions) to be fetched,
/// each of which can have multiple sources associated with it.
///
/// When adding the same key twice, the sources are merged by appending the newest
/// source to the front of the list of sources, and the contexts are merged by the
/// method defined in [`FetchPoolConfig`].
///
/// The queue items can be accessed only through its Iterator implementation.
/// Each item contains a FetchKey and one Source agent from which to fetch it.
/// Each time an item is obtained in this way, it is moved to the end of the list.
/// It is important to use the iterator lazily, and only take what is needed.
/// Accessing any item through iteration implies that a fetch was attempted.
#[derive(Clone)]
pub struct FetchPool {
    config: FetchConfig,
    state: ShareOpen<State>,
}

impl std::fmt::Debug for FetchPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.state
            .share_ref(|state| f.debug_struct("FetchPool").field("state", state).finish())
    }
}

/// Alias
pub type FetchConfig = Arc<dyn FetchPoolConfig>;

/// Host-defined details about how the fetch queue should function
pub trait FetchPoolConfig: 'static + Send + Sync {
    /// How long between successive item fetches, regardless of source?
    /// This gives a source a fair chance to respond before proceeding with a
    /// different source.
    ///
    /// The most conservative setting for this is `2 * tuning_params.implicit_timeout`,
    /// since that is the maximum amount of time a successful response can take.
    /// Lower values will give up early and may result in duplicate data sent if the
    /// response takes a long time to come back.
    fn item_retry_delay(&self) -> std::time::Duration {
        std::time::Duration::from_secs(90)
    }

    /// How long between successive fetches from a particular source, for a particular item?
    /// This protects us from wasting resources on a source which may be offline.
    /// This will eventually be replaced with an exponential backoff which will be
    /// tracked for this source across all items.
    fn source_retry_delay(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5 * 60)
    }

    /// When a fetch key is added twice, this determines how the two different contexts
    /// get reconciled.
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32;
}

/// The actual inner state of the FetchPool, from which items can be obtained
#[derive(Debug)]
pub struct State {
    /// Items ready to be fetched
    queue: LinkedHashMap<FetchKey, FetchPoolItem>,
}

#[allow(clippy::derivable_impls)]
impl Default for State {
    fn default() -> Self {
        Self {
            queue: Default::default(),
        }
    }
}

// TODO: move this to host, but for now, for convenience, we just use this one config
// for every queue
struct FetchPoolConfigBitwiseOr;

impl FetchPoolConfig for FetchPoolConfigBitwiseOr {
    fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
        a | b
    }
}

impl FetchPool {
    /// Constructor
    pub fn new(config: FetchConfig) -> Self {
        Self {
            config,
            state: ShareOpen::new(State::default()),
        }
    }

    /// Constructor, using only the "hardcoded" config (TODO: remove)
    pub fn new_bitwise_or() -> Self {
        Self {
            config: Arc::new(FetchPoolConfigBitwiseOr),
            state: ShareOpen::new(State::default()),
        }
    }

    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    pub fn push(&self, args: FetchPoolPush) {
        self.state.share_mut(|s| {
            tracing::debug!(
                "FetchPool (size = {}) item added: {:?}",
                s.queue.len() + 1,
                args
            );
            s.push(&*self.config, args);
        });
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    pub fn remove(&self, key: &FetchKey) -> Option<FetchPoolItem> {
        self.state.share_mut(|s| {
            let removed = s.remove(key);
            tracing::debug!(
                "FetchPool (size = {}) item removed: key={:?} val={:?}",
                s.queue.len(),
                key,
                removed
            );
            removed
        })
    }

    /// Get a list of the next items that should be fetched.
    pub fn get_items_to_fetch(&self) -> Vec<(FetchKey, KSpace, FetchSource, Option<FetchContext>)> {
        self.state.share_mut(|s| {
            let mut out = Vec::new();

            for (key, space, source, context) in s.iter_mut(&*self.config) {
                out.push((key, space, source, context));
            }

            out
        })
    }
}

impl State {
    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    pub fn push(&mut self, config: &dyn FetchPoolConfig, args: FetchPoolPush) {
        let FetchPoolPush {
            key,
            author,
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
                let item = FetchPoolItem {
                    sources,
                    space,
                    size,
                    context,
                    last_fetch: None,
                };
                e.insert(item);
            }
            Entry::Occupied(mut e) => {
                let v = e.get_mut();
                v.sources.0.insert(0, SourceRecord::new(source));
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
    pub fn iter_mut<'a>(&'a mut self, config: &'a dyn FetchPoolConfig) -> StateIter {
        StateIter {
            state: self,
            config,
        }
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    pub fn remove(&mut self, key: &FetchKey) -> Option<FetchPoolItem> {
        self.queue.remove(key)
    }

    /// Get a string summary of the queue's contents
    #[cfg(feature = "test_utils")]
    pub fn summary(&self) -> String {
        use human_repr::HumanCount;

        let table = self
            .queue
            .iter()
            .map(|(k, v)| {
                let key = match k {
                    FetchKey::Op(hash) => {
                        let h = hash.to_string();
                        format!("{}..{}", &h[0..4], &h[h.len() - 4..])
                    }
                    FetchKey::Region(_) => "[region]".to_string(),
                };

                let size = v.size.unwrap_or_default().get();
                format!(
                    "{:10}  {:^6} {:^6} {:>6}",
                    key,
                    v.sources.0.len(),
                    v.last_fetch
                        .map(|t| format!("{:?}", t.elapsed()))
                        .unwrap_or_else(|| "-".to_string()),
                    size.human_count_bytes(),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}\n{} items total", table, self.queue.len())
    }

    /// The heading to go along with the summary
    #[cfg(feature = "test_utils")]
    pub fn summary_heading() -> String {
        format!("{:10}  {:>6} {:>6} {}", "key", "#src", "last", "size")
    }
}

/// A mutable iterator over the FetchPool State
pub struct StateIter<'a> {
    state: &'a mut State,
    config: &'a dyn FetchPoolConfig,
}

impl<'a> Iterator for StateIter<'a> {
    type Item = (FetchKey, KSpace, FetchSource, Option<FetchContext>);

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
            let item_not_recently_fetched = item
                .last_fetch
                .map(|t| t.elapsed() >= self.config.item_retry_delay())
                .unwrap_or(true);
            if item_not_recently_fetched {
                if let Some(source) = item.sources.next(self.config.source_retry_delay()) {
                    let space = item.space.clone();
                    item.last_fetch = Some(Instant::now());
                    return Some((key, space, source, item.context));
                }
            }
        }
        None
    }
}

/// An item in the queue, corresponding to a single op or region to fetch
#[derive(Debug, PartialEq, Eq)]
pub struct FetchPoolItem {
    /// Known sources from whom we can fetch this item.
    /// Sources will always be tried in order.
    sources: Sources,
    /// The space to retrieve this op from
    space: KSpace,
    /// Approximate size of the item. If set, the item will be counted towards overall progress.
    size: Option<RoughInt>,
    /// Opaque user data specified by the host
    pub context: Option<FetchContext>,
    /// The last time we tried fetching this item from any source
    last_fetch: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq)]
struct SourceRecord {
    source: FetchSource,
    last_request: Option<Instant>,
}

impl SourceRecord {
    fn new(source: FetchSource) -> Self {
        Self {
            source,
            last_request: None,
        }
    }

    fn agent(agent: KAgent) -> Self {
        Self {
            source: FetchSource::Agent(agent),
            last_request: None,
        }
    }
}

/// Fetch item within the fetch queue state.
#[derive(Debug, PartialEq, Eq)]
struct Sources(Vec<SourceRecord>);

impl Sources {
    fn next(&mut self, interval: Duration) -> Option<FetchSource> {
        if let Some((i, agent)) = self
            .0
            .iter()
            .enumerate()
            .find(|(_, s)| {
                s.last_request
                    .map(|t| t.elapsed() >= interval)
                    .unwrap_or(true)
            })
            .map(|(i, s)| (i, s.source.clone()))
        {
            self.0[i].last_request = Some(Instant::now());
            self.0.rotate_left(i + 1);
            Some(agent)
        } else {
            None
        }
    }
}

/// A source to fetch from: either a node, or an agent on a node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FetchSource {
    /// An agent on a node
    Agent(KAgent),
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;
    use std::{sync::Arc, time::Duration};

    use kitsune_p2p_types::bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash, KitsuneSpace};

    use super::*;

    pub(super) struct Config(pub u32, pub u32);

    impl FetchPoolConfig for Config {
        fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
            (a + b).min(1)
        }

        fn item_retry_delay(&self) -> Duration {
            Duration::from_secs(self.0 as u64)
        }

        fn source_retry_delay(&self) -> Duration {
            Duration::from_secs(self.1 as u64)
        }
    }

    pub(super) fn key_op(n: u8) -> FetchKey {
        FetchKey::Op(Arc::new(KitsuneOpHash::new(vec![n; 36])))
    }

    pub(super) fn req(n: u8, context: Option<FetchContext>, source: FetchSource) -> FetchPoolPush {
        FetchPoolPush {
            key: key_op(n),
            author: None,
            context,
            space: space(0),
            source,
            size: None,
        }
    }

    pub(super) fn item(
        _cfg: &dyn FetchPoolConfig,
        sources: Vec<FetchSource>,
        context: Option<FetchContext>,
    ) -> FetchPoolItem {
        FetchPoolItem {
            sources: Sources(sources.into_iter().map(|s| SourceRecord::new(s)).collect()),
            space: Arc::new(KitsuneSpace::new(vec![0; 36])),
            context,
            size: None,
            last_fetch: None,
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

    #[tokio::test(start_paused = true)]
    async fn source_rotation() {
        let sec1 = Duration::from_secs(10);
        let mut ss = Sources(vec![
            SourceRecord {
                source: source(1),
                last_request: Some(Instant::now()),
            }
            .into(),
            SourceRecord {
                source: source(2),
                last_request: None,
            }
            .into(),
        ]);

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(ss.next(sec1), Some(source(2)));
        assert_eq!(ss.next(sec1), None);

        tokio::time::advance(Duration::from_secs(9)).await;

        assert_eq!(ss.next(sec1), Some(source(1)));

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(ss.next(sec1), Some(source(2)));
        // source 1 has already had its delay backed off to 20s
        // due to a retry, so it returns None
        assert_eq!(ss.next(sec1), None);

        tokio::time::advance(Duration::from_secs(20)).await;

        assert_eq!(ss.next(sec1), Some(source(1)));
        assert_eq!(ss.next(sec1), Some(source(2)));
        assert_eq!(ss.next(sec1), None);
    }

    #[test]
    fn queue_push() {
        let mut q = State::default();
        let c = Config(1, 1);

        // note: new sources get added to the front of the list
        q.push(&c, req(1, ctx(1), source(1)));
        q.push(&c, req(1, ctx(0), source(0)));

        q.push(&c, req(2, ctx(0), source(0)));

        let expected_ready = [
            (key_op(1), item(&c, sources(0..=1), ctx(1))),
            (key_op(2), item(&c, sources([0]), ctx(0))),
        ]
        .into_iter()
        .collect();

        assert_eq!(q.queue, expected_ready);
    }

    #[tokio::test(start_paused = true)]
    async fn queue_next() {
        let cfg = Config(1, 10);
        let mut q = {
            let mut queue = [
                (key_op(1), item(&cfg, sources(0..=2), ctx(1))),
                (key_op(2), item(&cfg, sources(1..=3), ctx(1))),
                (key_op(3), item(&cfg, sources(2..=4), ctx(1))),
            ];
            // Set the last_fetch time of one of the sources to something a bit earlier,
            // so it won't show up in next() right away
            queue[1].1.sources.0[1].last_request = Some(Instant::now() - Duration::from_secs(3));

            let queue = queue.into_iter().collect();
            State { queue }
        };

        // We can try fetching items one source at a time by waiting 1 sec in between

        assert_eq!(q.iter_mut(&cfg).count(), 3);

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(q.iter_mut(&cfg).count(), 3);

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(q.iter_mut(&cfg).count(), 2);

        // Wait for manually modified source to be ready
        // (5 + 1 + 1 + 3 = 10)
        tokio::time::advance(Duration::from_secs(5)).await;

        // The next (and only) item will be the one with the timestamp explicitly set
        assert_eq!(
            q.iter_mut(&cfg).collect::<Vec<_>>(),
            vec![(key_op(2), space(0), source(2), ctx(1))]
        );
        assert_eq!(q.iter_mut(&cfg).count(), 0);

        // wait long enough for some items to be retryable
        // (10 - 5 - 1 = 4)
        tokio::time::advance(Duration::from_secs(4)).await;

        assert_eq!(q.iter_mut(&cfg).count(), 3);
    }
}
