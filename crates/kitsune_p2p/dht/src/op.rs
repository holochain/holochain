use std::{borrow::Borrow, sync::Arc};

use crate::{
    coords::{SpacetimeCoords, Topology},
    hash::OpHash,
    region::RegionData,
};

pub use kitsune_p2p_dht_arc::DhtLocation as Loc;

pub use kitsune_p2p_timestamp::Timestamp;

/// TODO: mark this as for testing only. /// This is indeed the type that Holochain provides.
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

    /// Obviously only for testing
    pub fn fake(loc: Loc, timestamp: Timestamp, size: u32) -> Op {
        use crate::hash::fake_hash;
        Op::new(Self {
            loc,
            timestamp,
            size,
            hash: fake_hash().into(),
        })
    }
}

impl Borrow<Timestamp> for OpData {
    fn borrow(&self) -> &Timestamp {
        &self.timestamp
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

pub trait OpRegion<D>: PartialOrd + Ord {
    fn loc(&self) -> Loc;
    fn timestamp(&self) -> Timestamp;
    fn region_data(&self) -> D;

    fn coords(&self, topo: &Topology) -> SpacetimeCoords {
        SpacetimeCoords {
            space: topo.space_coord(self.loc()),
            time: topo.time_coord(self.timestamp()),
        }
    }
    fn region_tuple(&self, topo: &Topology) -> (SpacetimeCoords, D) {
        (self.coords(topo), self.region_data())
    }

    fn bound(timestamp: Timestamp, loc: Loc) -> Self;
}

impl OpRegion<RegionData> for OpData {
    fn loc(&self) -> Loc {
        self.loc
    }

    fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    fn region_data(&self) -> RegionData {
        RegionData {
            hash: self.hash.into(),
            size: self.size,
            count: 1,
        }
    }

    fn bound(timestamp: Timestamp, loc: Loc) -> Self {
        Self {
            loc,
            timestamp,
            size: 0,
            hash: [0; 32].into(),
        }
    }
}

pub type Op = Arc<OpData>;
