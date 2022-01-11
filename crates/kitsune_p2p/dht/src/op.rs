use std::sync::Arc;

use crate::{
    coords::{SpacetimeCoords, Topology},
    hash::OpHash,
    region::RegionData,
};

pub use kitsune_p2p_dht_arc::DhtLocation as Loc;

pub use kitsune_p2p_timestamp::Timestamp;

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

    pub fn to_tree_data(&self, q: &Topology) -> (SpacetimeCoords, RegionData) {
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

    /// Obviously only for testing
    pub fn fake(loc: u32, timestamp: i64, size: u32) -> Self {
        use crate::hash::fake_hash;
        Self {
            loc: Loc::from(loc),
            timestamp: Timestamp::from_micros(timestamp),
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
