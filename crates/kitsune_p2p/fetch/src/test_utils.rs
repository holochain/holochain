use crate::{FetchContext, FetchKey, FetchPoolPush, FetchSource};
use kitsune_p2p_types::bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash, KitsuneSpace};
use kitsune_p2p_types::dht::prelude::{
    RegionCoords, SpaceOffset, SpaceSegment, TimeOffset, TimeSegment,
};
use kitsune_p2p_types::{KOpHash, KSpace};
use std::sync::Arc;

pub fn test_key_hash(n: u8) -> KOpHash {
    Arc::new(KitsuneOpHash::new(vec![n; 36]))
}

pub fn test_key_op(n: u8) -> FetchKey {
    FetchKey::Op(test_key_hash(n))
}

pub fn test_key_region(n: u8) -> FetchKey {
    FetchKey::Region(RegionCoords {
        space: SpaceSegment {
            power: n,
            offset: SpaceOffset(0),
        },
        time: TimeSegment {
            power: n,
            offset: TimeOffset(0),
        },
    })
}

pub fn test_req_op(n: u8, context: Option<FetchContext>, source: FetchSource) -> FetchPoolPush {
    FetchPoolPush {
        key: test_key_op(n),
        author: None,
        context,
        space: test_space(0),
        source,
        size: None,
    }
}

pub fn test_req_region(n: u8, context: Option<FetchContext>, source: FetchSource) -> FetchPoolPush {
    FetchPoolPush {
        key: test_key_region(n),
        author: None,
        context,
        space: test_space(0),
        source,
        size: None,
    }
}

pub fn test_space(i: u8) -> KSpace {
    Arc::new(KitsuneSpace::new(vec![i; 36]))
}

pub fn test_source(i: u8) -> FetchSource {
    FetchSource::Agent(Arc::new(KitsuneAgent::new(vec![i; 36])))
}

pub fn test_sources(ix: impl IntoIterator<Item = u8>) -> Vec<FetchSource> {
    ix.into_iter().map(test_source).collect()
}

pub fn test_ctx(c: u32) -> Option<FetchContext> {
    Some(c.into())
}
