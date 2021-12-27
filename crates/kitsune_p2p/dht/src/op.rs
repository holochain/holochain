use std::sync::Arc;

use crate::{
    coords::{SpacetimeCoords, Topology},
    region_data::RegionData,
};

pub use kitsune_p2p_dht_arc::DhtLocation as Loc;

#[derive(
    Copy,
    Clone,
    Debug,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    derive_more::From,
    derive_more::AsRef,
    derive_more::Deref,
)]
pub struct Timestamp(i64);

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, derive_more::From)]
pub struct OpHash(pub [u8; 32]);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct OpData {
    pub loc: Loc,
    pub hash: OpHash,
    pub size: u32,
    pub timestamp: Timestamp,
}

impl OpData {
    pub fn loc(&self) -> Loc {
        self.loc
    }

    pub fn to_node(&self, q: &Topology) -> (SpacetimeCoords, RegionData) {
        let coords = SpacetimeCoords {
            space: q.space_coord(self.loc),
            time: q.time_coord(self.timestamp),
        };
        let data = RegionData {
            hash: self.hash.into(),
            size: self.size,
            count: 1,
        };
        (coords, data)
    }

    #[cfg(test)]
    pub fn fake(loc: Loc, timestamp: Timestamp, size: u32) -> Self {
        use crate::region_data::fake_hash;
        Self {
            loc,
            timestamp,
            size,
            hash: fake_hash().into(),
        }
    }
}

impl PartialOrd for OpData {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OpData {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.timestamp, &self.loc).cmp(&(&other.timestamp, &other.loc))
    }
}

pub type Op = Arc<OpData>;
