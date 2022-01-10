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
        let op0 = OpData {
            loc: Loc::from(*x0),
            timestamp: Timestamp::from_micros(*t0 as i64),
            size: 0,
            hash: [0; 32].into(),
        };
        let op1 = OpData {
            loc: Loc::from(*x1),
            timestamp: Timestamp::from_micros(*t1 as i64),
            size: 0,
            hash: [0; 32].into(),
        };
        self.0
            .range(op0..=op1)
            .filter(|o| *x0 <= o.loc().as_u32() && o.loc().as_u32() <= *x1)
            .cloned()
            .collect()
    }

    pub fn add_ops(&mut self, ops: impl Iterator<Item = Op>) {
        self.0.extend(ops)
    }
}
