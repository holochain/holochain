use std::collections::BTreeSet;

use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::{tx2::tx2_utils::Share, KAgent};
use linked_hash_map::{Entry, LinkedHashMap};

use crate::{FetchContext, FetchKey, FetchOptions, FetchRequest, FetchResponse, FetchResult};

pub struct FetchQueue(Share<FetchQueueState>);

type ContextMergeFn = Box<dyn Fn(&mut FetchContext, FetchContext) + Send + Sync + 'static>;

pub struct FetchQueueState {
    /// Items ready to be fetched
    ready: LinkedHashMap<FetchKey, FetchQueueItem>,
    /// Items for which a fetch has been initiated and we're waiting
    /// for the data.
    /// If the data times out, we can fall back to another source,
    /// or remove it from the queue (depending on the sources and FetchOptions).
    awaiting: BTreeSet<FetchQueueCurrentJob>,
    /// Function which knows how to merge two FetchContexts into one.
    context_merge_fn: ContextMergeFn,
}

#[derive(Debug, PartialEq, Eq)]
struct FetchQueueItem {
    /// Known sources from whom we can fetch this item.
    /// Sources will always be tried in order.
    sources: Vec<KAgent>,
    /// Options specified for this fetch job
    options: Option<FetchOptions>,
    /// If Some, this item is currently being fetched, and will timeout at the expiry time.
    /// If None, this is a ready job that has not been started.
    expiry: Option<Timestamp>,
    /// Opaque user data specified by the host
    context: Option<FetchContext>,
}

/// A fetch request which has been sent, whose results we are awaiting.
/// Ordering is based solely on the expiry timestamp.
#[derive(Debug, PartialEq, Eq)]
struct FetchQueueCurrentJob {
    expires: Timestamp,
    key: FetchKey,
    val: FetchQueueItem,
}

impl PartialOrd for FetchQueueCurrentJob {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.expires.partial_cmp(&other.expires) {
            Some(core::cmp::Ordering::Equal) => None,
            ord => ord,
        }
    }
}

impl FetchQueueState {
    pub fn new(
        context_merge_fn: impl Fn(&mut FetchContext, FetchContext) + Send + Sync + 'static,
    ) -> Self {
        Self {
            ready: Default::default(),
            awaiting: Default::default(),
            context_merge_fn: Box::new(context_merge_fn),
        }
    }

    pub fn push(&mut self, source: KAgent, request: FetchRequest) -> () {
        let FetchRequest {
            key,
            author,
            options,
            context,
        } = request;

        match self.ready.entry(key) {
            Entry::Vacant(e) => {
                let sources = if let Some(author) = author {
                    vec![source, author]
                } else {
                    vec![source]
                };
                let item = FetchQueueItem {
                    sources,
                    options,
                    context,
                    expiry: None,
                };
                e.insert(item);
            }
            Entry::Occupied(mut e) => {
                let v = e.get_mut();
                v.sources.push(source);
                v.options = options;
                match (&mut v.context, context) {
                    (Some(a), Some(b)) => (self.context_merge_fn)(a, b),
                    (a, b) => v.context = a.take().and(b),
                }
            }
        }
        // - is the key already in the queue? If so, update the item in the queue with any extra info, like an additional source, or an update to the FetchOptions.
        // - is the key already being fetched? If so, update its info in the `in_flight` set, for instance if the key is already waiting to be fetched due to gossip, but then a publish request comes in for the same data.
        // - is the key in limbo? if so, register any extra post-integration instructions (like publishing author)
        // - is the key integrated? then go straight to the post-integration phase.
    }

    async fn await_item(&mut self, key: &FetchKey) -> FetchResult<Option<FetchResponse>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use kitsune_p2p_types::bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash};

    use super::*;

    #[test]
    fn push_queue() {
        let agents = vec![
            Arc::new(KitsuneAgent::new(vec![0; 36])),
            Arc::new(KitsuneAgent::new(vec![1; 36])),
            Arc::new(KitsuneAgent::new(vec![2; 36])),
        ];
        let mut q = FetchQueueState::new(|_, _| {});

        let key_op = |n| FetchKey::Op {
            op_hash: Arc::new(KitsuneOpHash::new(vec![n; 36])),
        };
        let req = |n| FetchRequest::with_key(key_op(n));
        let item = |sources| FetchQueueItem {
            sources,
            options: Default::default(),
            context: None,
            expiry: None,
        };

        q.push(agents[0].clone(), req(1));
        q.push(agents[0].clone(), req(2));
        q.push(agents[1].clone(), req(1));

        let expected_ready = [
            (key_op(1), item(agents[0..=1].to_vec())),
            (key_op(2), item(vec![agents[0].clone()])),
        ]
        .into_iter()
        .collect();

        assert_eq!(q.ready, expected_ready);
    }
}
