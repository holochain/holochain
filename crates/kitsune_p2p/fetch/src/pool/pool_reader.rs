use std::collections::HashSet;

use kitsune_p2p_types::KSpace;

use crate::FetchPool;

/// Read-only access to the queue
#[derive(Clone, derive_more::From)]
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

    /// Get a concise textual summary of the contents of the FetchPool
    pub fn summary(&self) -> String {
        self.0.state.share_ref(|s| s.summary())
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
    use std::sync::Arc;

    use kitsune_p2p_types::tx2::tx2_utils::ShareOpen;

    use crate::{pool::tests::*, State};

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
            FetchPoolReader(FetchPool {
                config: Arc::new(cfg),
                state: ShareOpen::new(State { queue }),
            })
        };
        println!("{}", State::summary_heading());
        println!("{}", q.summary());
        let info = q.info([space(0)].into_iter().collect());
        // The item without a size is not returned.
        assert_eq!(info.num_ops_to_fetch, 2);
        assert_eq!(info.op_bytes_to_fetch, 1100);
    }
}
