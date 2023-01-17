use std::{collections::HashSet, sync::Arc};

use kitsune_p2p_types::{tx2::tx2_utils::ShareOpen, KSpace};

use crate::FetchQueue;

/// Read-only access to the queue
#[derive(Clone)]
pub struct FetchQueueReader {
    queue: FetchQueue,
    max_info: Arc<ShareOpen<FetchQueueInfo>>,
}

impl FetchQueueReader {
    /// Constructor
    pub fn new(queue: FetchQueue) -> Self {
        Self {
            queue,
            max_info: Arc::new(ShareOpen::new(Default::default())),
        }
    }
    /// Get info about the queue, filtered by space
    pub fn info(&self, spaces: HashSet<KSpace>) -> FetchQueueInfoStateful {
        let (count, bytes) = self.queue.state.share_ref(|s| {
            s.queue
                .values()
                .filter(|v| spaces.contains(&v.space))
                .filter_map(|v| v.size.map(|s| s.get()))
                .fold((0, 0), |(c, s), t| (c + 1, s + t))
        });

        let max = self.max_info.share_mut(|i| {
            if count > i.num_ops_to_fetch {
                i.num_ops_to_fetch = count;
            }
            if bytes > i.op_bytes_to_fetch {
                i.op_bytes_to_fetch = bytes;
            }
            if count == 0 && bytes == 0 {
                i.num_ops_to_fetch = 0;
                i.op_bytes_to_fetch = 0;
            }
            i.clone()
        });

        let current = FetchQueueInfo {
            op_bytes_to_fetch: bytes,
            num_ops_to_fetch: count,
        };
        FetchQueueInfoStateful { current, max }
    }
}

impl std::fmt::Debug for FetchQueueReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchQueueReader")
            .field("queue", &self.queue)
            .field("max_info", &self.max_info.share_ref(|i| i.clone()))
            .finish()
    }
}

/// Info about the fetch queue
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FetchQueueInfo {
    /// Total number of bytes expected to be received through fetches
    pub op_bytes_to_fetch: usize,

    /// Total number of ops expected to be received through fetches
    pub num_ops_to_fetch: usize,
}

/// The instantaneous and accumulated max FetchQueueInfo
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FetchQueueInfoStateful {
    /// The instantaneous info
    pub current: FetchQueueInfo,
    /// The max info since the last time it went to zero
    pub max: FetchQueueInfo,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kitsune_p2p_types::tx2::tx2_utils::ShareOpen;

    use crate::{queue::tests::*, State};

    use super::*;

    #[test]
    fn queue_info() {
        let cfg = Config(1, 1);
        let q = {
            let mut queue = [
                (key_op(1), item(&cfg, sources(0..=2), ctx(1))),
                (key_op(2), item(&cfg, sources(1..=3), ctx(1))),
                (key_op(3), item(&cfg, sources(2..=4), ctx(1))),
            ];

            queue[0].1.size = Some(100.into());
            queue[1].1.size = Some(1000.into());

            let queue = queue.into_iter().collect();
            FetchQueueReader {
                queue: FetchQueue {
                    config: Arc::new(cfg),
                    state: ShareOpen::new(State { queue }),
                },
                max_info: Arc::new(ShareOpen::new(Default::default())),
            }
        };
        let info = q.info([space(0)].into_iter().collect());
        // The item without a size is not returned.
        assert_eq!(info.current.num_ops_to_fetch, 2);
        assert_eq!(info.current.op_bytes_to_fetch, 1100);
    }
}
