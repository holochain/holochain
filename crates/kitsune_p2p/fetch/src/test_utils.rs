//! Test utilities for the fetch crate.

use crate::{FetchContext, FetchKey, FetchPoolPush, FetchSource, TransferMethod};
use kitsune_p2p_types::bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash, KitsuneSpace};
use kitsune_p2p_types::{KOpHash, KSpace};
use std::sync::Arc;

/// Create a sample op hash.
pub fn test_key_hash(n: u8) -> KOpHash {
    Arc::new(KitsuneOpHash::new(vec![n; 36]))
}

/// Create a sample FetchKey::Op.
pub fn test_key_op(n: u8) -> FetchKey {
    FetchKey::Op(test_key_hash(n))
}

/// Create a sample FetchPoolPush keyed with a FetchKey::Op.
pub fn test_req_op(n: u8, context: Option<FetchContext>, source: FetchSource) -> FetchPoolPush {
    FetchPoolPush {
        key: test_key_op(n),
        author: None,
        context,
        space: test_space(0),
        source,
        size: None,
        transfer_method: TransferMethod::Gossip,
    }
}

/// Create a sample space.
pub fn test_space(i: u8) -> KSpace {
    Arc::new(KitsuneSpace::new(vec![i; 36]))
}

/// Create a sample FetchSource.
pub fn test_source(i: u8) -> FetchSource {
    FetchSource::Agent(Arc::new(KitsuneAgent::new(vec![i; 36])))
}

/// Create multiple sample [FetchSource]s at once.
pub fn test_sources(ix: impl IntoIterator<Item = u8>) -> Vec<FetchSource> {
    ix.into_iter().map(test_source).collect()
}

/// Create a sample [FetchContext].
pub fn test_ctx(c: u32) -> Option<FetchContext> {
    Some(c.into())
}
