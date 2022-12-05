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
                    .map(|v| v.size.unwrap_or_default().get())
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
