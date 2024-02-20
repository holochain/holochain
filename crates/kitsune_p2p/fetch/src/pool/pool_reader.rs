use std::collections::HashSet;

use kitsune_p2p_types::{fetch_pool::FetchPoolInfo, KSpace};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use crate::{pool::tests::*, State};
    use kitsune_p2p_types::tx2::tx2_utils::ShareOpen;
    use std::sync::Arc;

    #[test]
    fn queue_info_empty() {
        let fetch_pool_reader = FetchPoolReader(FetchPool {
            config: Arc::new(TestFetchConfig(1, 1)),
            state: ShareOpen::new(Default::default()),
        });

        let info = fetch_pool_reader.info([test_space(0), test_space(1)].into_iter().collect());
        assert_eq!(0, info.op_bytes_to_fetch);
        assert_eq!(0, info.num_ops_to_fetch);
    }

    #[test]
    fn queue_info_fetch_no_spaces() {
        let cfg = Arc::new(TestFetchConfig(1, 1));
        let q = {
            let mut queue = [(
                test_key_op(1),
                item(cfg.clone(), test_sources(0..=2), test_ctx(1)),
            )];

            queue[0].1.size = Some(100.into());

            let queue = queue.into_iter().collect();
            FetchPoolReader(FetchPool {
                config: cfg,
                state: ShareOpen::new(State {
                    queue,
                    ..Default::default()
                }),
            })
        };

        let info = q.info([].into_iter().collect());

        assert_eq!(0, info.num_ops_to_fetch);
        assert_eq!(0, info.op_bytes_to_fetch);
    }

    #[test]
    fn queue_info() {
        let cfg = Arc::new(TestFetchConfig(1, 1));
        let q = {
            let mut queue = [
                (
                    test_key_op(1),
                    item(cfg.clone(), test_sources(0..=2), test_ctx(1)),
                ),
                (
                    test_key_op(2),
                    item(cfg.clone(), test_sources(1..=3), test_ctx(1)),
                ),
                (
                    test_key_op(3),
                    item(cfg.clone(), test_sources(2..=4), test_ctx(1)),
                ),
            ];

            queue[0].1.size = Some(100.into());
            queue[1].1.size = Some(1000.into());

            let queue = queue.into_iter().collect();
            FetchPoolReader(FetchPool {
                config: cfg,
                state: ShareOpen::new(State {
                    queue,
                    ..Default::default()
                }),
            })
        };

        let info = q.info([test_space(0)].into_iter().collect());
        // The item without a size is not returned.
        assert_eq!(info.num_ops_to_fetch, 2);
        assert_eq!(info.op_bytes_to_fetch, 1100);
    }

    #[test]
    fn queue_info_filter_spaces() {
        let cfg = Arc::new(TestFetchConfig(1, 1));
        let q = {
            let mut item_for_space_1 = item(cfg.clone(), test_sources(0..=2), test_ctx(1));
            item_for_space_1.space = test_space(1);
            item_for_space_1.size = Some(100.into());

            let mut item_for_space_2 = item(cfg.clone(), test_sources(0..=2), test_ctx(1));
            item_for_space_2.space = test_space(2);
            item_for_space_2.size = Some(500.into());

            let queue = [
                (test_key_op(1), item_for_space_1),
                (test_key_op(2), item_for_space_2),
            ];

            let queue = queue.into_iter().collect();
            FetchPoolReader(FetchPool {
                config: cfg,
                state: ShareOpen::new(State {
                    queue,
                    ..Default::default()
                }),
            })
        };

        let info_space_1 = q.info([test_space(1)].into_iter().collect());
        assert_eq!(info_space_1.num_ops_to_fetch, 1);
        assert_eq!(info_space_1.op_bytes_to_fetch, 100);

        let info_space_2 = q.info([test_space(2)].into_iter().collect());
        assert_eq!(info_space_2.num_ops_to_fetch, 1);
        assert_eq!(info_space_2.op_bytes_to_fetch, 500);

        let info_space_2 = q.info([test_space(1), test_space(2)].into_iter().collect());
        assert_eq!(info_space_2.num_ops_to_fetch, 2);
        assert_eq!(info_space_2.op_bytes_to_fetch, 600);
    }
}
