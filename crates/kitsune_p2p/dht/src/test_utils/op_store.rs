use std::collections::BTreeSet;

use crate::{
    op::{Op, OpData, Timestamp},
    region::RegionBounds,
    Loc,
};

#[derive(Default)]
pub struct OpStore(BTreeSet<Op>);

impl OpStore {
    /// Return all ops within a region of spacetime
    pub fn query_region(&self, region: &RegionBounds) -> Vec<Op> {
        let (x0, x1) = region.x;
        let (t0, t1) = region.t;
        let op0 = op_bound(Timestamp::from_micros(*t0 as i64), Loc::from(*x0));
        let op1 = op_bound(Timestamp::from_micros(*t1 as i64), Loc::from(*x1));
        self.0
            .range(op0..=op1)
            .filter(|o| *x0 <= o.loc().as_u32() && o.loc().as_u32() <= *x1)
            .cloned()
            .collect()
    }

    pub fn max_timestamp(&self) -> Option<Timestamp> {
        self.0
            .range(op_bound(Timestamp::MIN, Loc::MIN)..=op_bound(Timestamp::MAX, Loc::MAX))
            .next_back()
            .map(|d| d.timestamp)
    }

    pub fn add_ops(&mut self, ops: impl Iterator<Item = Op>) {
        self.0.extend(ops)
    }
}

fn op_bound(timestamp: Timestamp, loc: Loc) -> OpData {
    OpData {
        loc,
        timestamp,
        size: 0,
        hash: [0; 32].into(),
    }
}
