use std::collections::HashSet;

use kitsune_p2p_types::KSpace;

use crate::FetchPool;

/// Read-only access to the queue
#[derive(Debug, Clone, derive_more::From)]
pub struct FetchPoolReader(FetchPool);

impl FetchPoolReader {
    /// Get info about the queue, filtered by space
    pub fn info(&self, spaces: HashSet<KSpace>) -> FetchPoolInfo {
        let (count, bytes) = self.0.state.share_ref(|s| {
            s.queue
                .values()
                .filter(|v| spaces.contains(&v.space))
                .filter_map(|v| v.size.map(|s| s.get()))
                .fold((0, 0), |(c, s), t| (c + 1, s + t))
        });

        FetchPoolInfo {
            op_bytes_to_fetch: bytes,
            num_ops_to_fetch: count,
        }
    }
}

/// Info about the fetch queue
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FetchPoolInfo {
    /// Total number of bytes expected to be received through fetches
    pub op_bytes_to_fetch: usize,

    /// Total number of ops expected to be received through fetches
    pub num_ops_to_fetch: usize,
}

#[cfg(test)]
mod tests {
    use linked_hash_map::LinkedHashMap;
    use std::sync::Arc;

    use kitsune_p2p_types::tx2::tx2_utils::ShareOpen;

    use crate::{pool::tests::*, State};

    use super::*;

    #[test]
    fn queue_info_empty() {
        let fetch_pool_reader = FetchPoolReader(FetchPool {
            config: Arc::new(Config(1, 1)),
            state: ShareOpen::new(State {
                queue: LinkedHashMap::new(),
            }),
        });

        let info = fetch_pool_reader.info([space(0), space(1)].into_iter().collect());
        assert_eq!(0, info.op_bytes_to_fetch);
        assert_eq!(0, info.num_ops_to_fetch);
    }

    #[test]
    fn queue_info_fetch_no_spaces() {
        let cfg = Config(1, 1);
        let q = {
            let mut queue = [(key_op(1), item(&cfg, sources(0..=2), ctx(1)))];

            queue[0].1.size = Some(100.into());

            let queue = queue.into_iter().collect();
            FetchPoolReader(FetchPool {
                config: Arc::new(cfg),
                state: ShareOpen::new(State { queue }),
            })
        };

        let info = q.info([].into_iter().collect());

        assert_eq!(0, info.num_ops_to_fetch);
        assert_eq!(0, info.op_bytes_to_fetch);
    }

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
            FetchPoolReader(FetchPool {
                config: Arc::new(cfg),
                state: ShareOpen::new(State { queue }),
            })
        };

        let info = q.info([space(0)].into_iter().collect());
        // The item without a size is not returned.
        assert_eq!(info.num_ops_to_fetch, 2);
        assert_eq!(info.op_bytes_to_fetch, 1100);
    }

    #[test]
    fn queue_info_filter_spaces() {
        let cfg = Config(1, 1);
        let q = {
            let mut item_for_space_1 = item(&cfg, sources(0..=2), ctx(1));
            item_for_space_1.space = space(1);
            item_for_space_1.size = Some(100.into());

            let mut item_for_space_2 = item(&cfg, sources(0..=2), ctx(1));
            item_for_space_2.space = space(2);
            item_for_space_2.size = Some(500.into());

            let queue = [(key_op(1), item_for_space_1), (key_op(2), item_for_space_2)];

            let queue = queue.into_iter().collect();
            FetchPoolReader(FetchPool {
                config: Arc::new(cfg),
                state: ShareOpen::new(State { queue }),
            })
        };

        let info_space_1 = q.info([space(1)].into_iter().collect());
        assert_eq!(info_space_1.num_ops_to_fetch, 1);
        assert_eq!(info_space_1.op_bytes_to_fetch, 100);

        let info_space_2 = q.info([space(2)].into_iter().collect());
        assert_eq!(info_space_2.num_ops_to_fetch, 1);
        assert_eq!(info_space_2.op_bytes_to_fetch, 500);

        let info_space_2 = q.info([space(1), space(2)].into_iter().collect());
        assert_eq!(info_space_2.num_ops_to_fetch, 2);
        assert_eq!(info_space_2.op_bytes_to_fetch, 600);
    }
}
