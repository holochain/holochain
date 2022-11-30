use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use kitsune_p2p_types::{tx2::tx2_utils::Share, KAgent, KSpace};
use linked_hash_map::{Entry, LinkedHashMap};

use crate::{FetchContext, FetchKey, FetchOptions, FetchRequest};

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

/// Alias
pub type FetchConfig = Arc<dyn FetchQueueConfig>;

/// Host-defined details about how the fetch queue should function
pub trait FetchQueueConfig: 'static + Send + Sync {
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
struct Sources(Vec<Source>);

#[derive(Debug, PartialEq, Eq)]
struct FetchQueueItem {
    /// Known sources from whom we can fetch this item.
    /// Sources will always be tried in order.
    sources: Sources,
    /// The space to retrieve this op from
    space: KSpace,
    /// Options specified for this fetch job
    options: Option<FetchOptions>,
    /// Opaque user data specified by the host
    context: Option<FetchContext>,
}

#[derive(Debug, PartialEq, Eq)]
struct Source {
    agent: KAgent,
    last_fetch: Option<Instant>,
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
    pub fn push(&mut self, request: FetchRequest, space: KSpace, agent: KAgent) {
        self.state
            .share_mut(|s, _| Ok(s.push(&*self.config, request, space, agent)))
            .expect("no error");
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    pub fn remove(&mut self, key: &FetchKey) {
        self.state
            .share_mut(|s, _| Ok(s.remove(key)))
            .expect("no error");
    }
}

impl State {
    /// Add an item to the queue.
    /// If the FetchKey does not already exist, add it to the end of the queue.
    /// If the FetchKey exists, add the new source and merge the context in, without
    /// changing the position in the queue.
    pub fn push(
        &mut self,
        config: &dyn FetchQueueConfig,
        request: FetchRequest,
        space: KSpace,
        agent: KAgent,
    ) {
        let FetchRequest {
            key,
            author,
            options,
            context,
        } = request;

        match self.queue.entry(key) {
            Entry::Vacant(e) => {
                let sources = if let Some(author) = author {
                    Sources(vec![Source::new(agent), Source::new(author)])
                } else {
                    Sources(vec![Source::new(agent)])
                };
                let item = FetchQueueItem {
                    sources,
                    space,
                    options,
                    context,
                };
                e.insert(item);
            }
            Entry::Occupied(mut e) => {
                let v = e.get_mut();
                v.sources.0.insert(0, Source::new(agent));
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
    pub fn remove(&mut self, key: &FetchKey) {
        self.queue.remove(key);
    }
}

impl<'a> Iterator for StateIter<'a> {
    type Item = (FetchKey, KSpace, KAgent);

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
            if let Some(agent) = item.sources.next(self.interval) {
                let space = item.space.clone();
                return Some((key, space, agent));
            }
        }
        None
    }
}

impl Source {
    fn new(agent: KAgent) -> Self {
        Self {
            agent,
            last_fetch: None,
        }
    }
}

impl Sources {
    fn next(&mut self, interval: Duration) -> Option<KAgent> {
        if let Some((i, agent)) = self
            .0
            .iter()
            .enumerate()
            .find(|(_, s)| s.last_fetch.map(|t| t.elapsed() > interval).unwrap_or(true))
            .map(|(i, s)| (i, s.agent.clone()))
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

    struct Config;

    impl FetchQueueConfig for Config {
        fn merge_fetch_contexts(&self, a: u32, b: u32) -> u32 {
            (a + b).min(1)
        }
    }

    fn key_op(n: u8) -> FetchKey {
        FetchKey::Op {
            op_hash: Arc::new(KitsuneOpHash::new(vec![n; 36])),
        }
    }

    fn req(n: u8, c: Option<FetchContext>) -> FetchRequest {
        FetchRequest::with_key(key_op(n), c)
    }

    fn item(sources: Vec<KAgent>, context: Option<FetchContext>) -> FetchQueueItem {
        FetchQueueItem {
            sources: Sources(sources.into_iter().map(Source::new).collect()),
            space: Arc::new(KitsuneSpace::new(vec![0; 36])),
            options: Default::default(),
            context,
        }
    }

    fn space(i: u8) -> KSpace {
        Arc::new(KitsuneSpace::new(vec![i; 36]))
    }
    fn agent(i: u8) -> KAgent {
        Arc::new(KitsuneAgent::new(vec![i; 36]))
    }
    fn agents(ix: impl IntoIterator<Item = u8>) -> Vec<KAgent> {
        ix.into_iter().map(agent).collect()
    }
    fn ctx(c: u32) -> Option<FetchContext> {
        Some(c.into())
    }

    #[test]
    fn sources() {
        let mut ss = Sources(vec![
            Source {
                agent: agent(1),
                last_fetch: Some(Instant::now()),
            },
            Source {
                agent: agent(2),
                last_fetch: None,
            },
        ]);

        assert_eq!(ss.next(Duration::from_secs(10)), Some(agent(2)));
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_nanos(1)), Some(agent(1)));
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_nanos(1)), Some(agent(2)));
        assert_eq!(ss.next(Duration::from_nanos(1)), Some(agent(1)));
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_secs(10)), None);
        assert_eq!(ss.next(Duration::from_secs(10)), None);
    }

    #[test]
    fn queue_push() {
        let mut q = State::default();
        let c = Config;

        // note: new sources get added to the front of the list
        q.push(&c, req(1, ctx(1)), space(0), agent(1));
        q.push(&c, req(1, ctx(0)), space(0), agent(0));

        q.push(&c, req(2, ctx(0)), space(0), agent(0));

        let expected_ready = [
            (key_op(1), item(agents(0..=1), ctx(1))),
            (key_op(2), item(agents([0]), ctx(0))),
        ]
        .into_iter()
        .collect();

        assert_eq!(q.queue, expected_ready);
    }

    #[test]
    fn queue_next() {
        let mut q = {
            let mut queue = [
                (key_op(1), item(agents(0..=2), ctx(1))),
                (key_op(2), item(agents(1..=3), ctx(1))),
                (key_op(3), item(agents(2..=4), ctx(1))),
            ];
            // Set the last_fetch time of one of the sources, so it won't show up in next() right away
            queue[1].1.sources.0[1].last_fetch = Some(Instant::now() - Duration::from_secs(3));

            let queue = queue.into_iter().collect();
            State { queue }
        };
        assert_eq!(q.iter_mut(Duration::from_secs(10)).count(), 8);

        // The next (and only) item will be the one with the timestamp explicitly set
        assert_eq!(
            q.iter_mut(Duration::from_secs(1)).collect::<Vec<_>>(),
            vec![(key_op(2), space(0), agent(2))]
        );

        // When traversing the entire queue again, the "special" item is still the last one.
        let items: Vec<_> = q.iter_mut(Duration::from_millis(0)).take(9).collect();
        assert_eq!(items[8], (key_op(2), space(0), agent(2)));
    }
}
