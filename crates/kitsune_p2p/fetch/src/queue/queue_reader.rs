use std::collections::HashSet;

use kitsune_p2p_types::KSpace;

use crate::FetchQueue;

/// Read-only access to the queue
#[derive(Clone, derive_more::From)]
pub struct FetchQueueReader(FetchQueue);

impl FetchQueueReader {
    /// Get info about the queue, filtered by space
    pub fn info(&self, spaces: HashSet<KSpace>) -> FetchQueueInfo {
        let (count, bytes) = self
            .0
            .state
            .share_ref(|s| {
                Ok(s.queue
                    .values()
                    .filter(|v| spaces.contains(&v.space))
                    .filter_map(|v| v.size.map(|s| s.get()))
                    .fold((0, 0), |(c, s), t| (c + 1, s + t)))
            })
            .unwrap();
        FetchQueueInfo {
            op_bytes_to_fetch: bytes,
            num_ops_to_fetch: count,
        }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kitsune_p2p_types::tx2::tx2_utils::Share;

    use crate::{queue::tests::*, State};

    use super::*;

    #[test]
    fn queue_info() {
        let q = {
            let mut queue = [
                (key_op(1), item(sources(0..=2), ctx(1))),
                (key_op(2), item(sources(1..=3), ctx(1))),
                (key_op(3), item(sources(2..=4), ctx(1))),
            ];

            queue[0].1.size = Some(100.into());
            queue[1].1.size = Some(1000.into());

            let queue = queue.into_iter().collect();
            FetchQueueReader(FetchQueue {
                config: Arc::new(Config),
                state: Share::new(State { queue }),
            })
        };
        let info = q.info([space(0)].into_iter().collect());
        // The item without a size is not returned.
        assert_eq!(info.num_ops_to_fetch, 2);
        assert_eq!(info.op_bytes_to_fetch, 1100);
    }
}
