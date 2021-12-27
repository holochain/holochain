use std::collections::BTreeSet;

use crate::{
    coords::QuantumParams,
    fingerprint::SpacetimeCoords,
    op::{Loc, Op, OpData, Timestamp},
};

#[derive(Default)]
pub struct OpStore(BTreeSet<Op>);

pub struct SpacetimeRegion {
    pub x: (Loc, Loc),
    pub t: (Timestamp, Timestamp),
}

impl SpacetimeRegion {
    fn from_coords(params: &QuantumParams, coords: SpacetimeCoords) -> Self {
        let SpacetimeCoords(space, time) = coords;
        let (x0, x1) = space.bounds();
        let (t0, t1) = time.bounds(params);
        Self {
            x: (x0, x1),
            t: (t0, t1),
        }
    }
}

impl OpStore {
    /// Return all ops within a region of spacetime
    pub fn region<'a>(&'a self, region: SpacetimeRegion) -> Vec<Op> {
        let (x0, x1) = region.x;
        let (t0, t1) = region.t;
        let op0 = OpData {
            loc: x0,
            timestamp: t0,
            size: 0,
            hash: [0; 32].into(),
        };
        let op1 = OpData {
            loc: x1,
            timestamp: t1,
            size: 0,
            hash: [0; 32].into(),
        };
        self.0
            .range(op0..=op1)
            .filter(|o| x0 <= o.loc() && o.loc() <= x1)
            .cloned()
            .collect()
    }

    pub fn add_ops(&mut self, ops: impl Iterator<Item = Op>) {
        self.0.extend(ops)
    }
}
