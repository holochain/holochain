#![allow(unused_variables)]
#![allow(dead_code)]
#![warn(missing_docs)]

use std::{
    collections::BTreeSet,
    ops::Add,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{stream::FuturesUnordered, Future, FutureExt};
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::{tx2::tx2_utils::Share, KAgent, KSpace, KitsuneError, KitsuneResult};
use linked_hash_map::{Entry, LinkedHashMap};

use crate::{FetchContext, FetchError, FetchKey, FetchOptions, FetchRequest, FetchResult};

/// Max number of queue items to check on each `next()` poll
const NUM_ITEMS_PER_POLL: usize = 100;

#[derive(Clone)]
pub struct FetchQueue {
    config: Arc<dyn FetchQueueConfig>,
    state: Share<State>,
}

type ContextMergeFn = Box<dyn Fn(u32, u32) -> u32 + Send + Sync + 'static>;

pub trait FetchQueueConfig {
    fn fetch(&self, key: FetchKey, source: KAgent);

    fn merge_contexts(&self, a: u32, b: u32) -> u32;
}

#[derive(Default, Debug)]
struct State {
    /// Items ready to be fetched
    queue: LinkedHashMap<FetchKey, FetchQueueItem>,
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

impl State {
    fn push(
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
                    (Some(a), Some(b)) => Some(config.merge_contexts(*a, *b).into()),
                    (a, b) => a.and(b),
                }
            }
        }
        // TODO:
        // - [x] is the key already in the queue? If so, update the item in the queue with any extra info, like an additional source, or an update to the FetchOptions.
        // - [x] is the key already being fetched? If so, update its info in the `in_flight` set, for instance if the key is already waiting to be fetched due to gossip, but then a publish request comes in for the same data.
        // - [ ] is the key in limbo? if so, register any extra post-integration instructions (like publishing author)
        // - [ ] is the key integrated? then go straight to the post-integration phase.
    }

    fn next(&mut self, interval: Duration) -> Option<(FetchKey, KSpace, KAgent)> {
        let keys: Vec<_> = self
            .queue
            .keys()
            .take(NUM_ITEMS_PER_POLL)
            .cloned()
            .collect();
        for key in keys {
            let item = self.queue.get_refresh(&key)?;
            if let Some(agent) = item.sources.next(interval) {
                let space = item.space.clone();
                return Some((key, space, agent));
            }
        }
        None
    }

    /// When an item has been successfully fetched, we can remove it from the queue.
    fn remove(&mut self, key: &FetchKey) {
        self.queue.remove(key);
    }

    fn poll(&mut self) {}
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;
    use std::{sync::Arc, time::Duration};

    use kitsune_p2p_types::bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash, KitsuneSpace};

    use super::*;

    struct Config;

    impl FetchQueueConfig for Config {
        fn fetch(&self, key: FetchKey, source: KAgent) {
            // noop
        }

        fn merge_contexts(&self, a: u32, b: u32) -> u32 {
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
        let items: Vec<_> = std::iter::from_fn(|| q.next(Duration::from_secs(10))).collect();
        assert_eq!(items.len(), 8);

        // The next item will be the one with the timestamp explicitly set
        assert_eq!(
            q.next(Duration::from_secs(1)),
            Some((key_op(2), space(0), agent(2)))
        );
        // No more items to fetch for now
        assert_eq!(q.next(Duration::from_secs(1)), None);

        // When traversing the entire queue again, the "special" item is still the last one.
        let items: Vec<_> = std::iter::from_fn(|| q.next(Duration::from_millis(0)))
            .take(9)
            .collect();
        assert_eq!(items[8], (key_op(2), space(0), agent(2)));

        let c = Config;
    }
}
