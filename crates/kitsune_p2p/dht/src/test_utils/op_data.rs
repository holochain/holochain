use std::{borrow::Borrow, sync::Arc};

use kitsune_p2p_timestamp::Timestamp;

use crate::{hash::OpHash, prelude::OpRegion, region::RegionData, Loc};

/// TODO: mark this as for testing only.
/// This is indeed the type that Holochain provides.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct OpData {
    /// The DhtLocation
    pub loc: Loc,
    /// The hash of the op
    pub hash: OpHash,
    /// The size in bytes of the op data
    pub size: u32,
    /// The timestamp that the op was created
    pub timestamp: Timestamp,
}

impl OpData {
    /// Accessor
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

/// Alias for op
pub type Op = Arc<OpData>;
