use crate::{
    op::OpRegion,
    persistence::AccessOpStore,
    prelude::{RegionCoords, RegionSet, RegionSetLtcs},
    spacetime::{GossipParams, Topology},
    region::{RegionData, RegionDataConstraints},
};
use futures::future::FutureExt;
use std::{collections::BTreeSet, ops::Bound, sync::Arc};

use super::op_data::OpData;

/// An in-memory implementation of a node's op store
#[derive(Clone)]
pub struct OpStore<O: OpRegion<D> = OpData, D: RegionDataConstraints = RegionData> {
    pub(crate) topo: Topology,
    pub(crate) ops: BTreeSet<Arc<O>>,
    pub(crate) _region_set: RegionSet<D>,
    pub(crate) gossip_params: GossipParams,
}

impl<D: RegionDataConstraints, O: OpRegion<D>> OpStore<O, D> {
    /// Construct an empty store
    pub fn new(topo: Topology, gossip_params: GossipParams) -> Self {
        Self {
            topo,
            ops: Default::default(),
            _region_set: RegionSetLtcs::empty().into(),
            gossip_params,
        }
    }
}

impl<D: RegionDataConstraints, O: OpRegion<D>> AccessOpStore<O, D> for OpStore<O, D> {
    fn query_op_data(&self, region: &RegionCoords) -> Vec<Arc<O>> {
        let region = region.to_bounds(self.topo());
        let (x0, x1) = region.x;
        let (t0, t1) = region.t;
        let op0 = O::bound(t0, x0);
        let op1 = O::bound(t1, x0);
        self.ops
            .range((Bound::Included(op0), Bound::Included(op1)))
            .filter(|o| x0 <= o.loc() && o.loc() <= x1)
            .cloned()
            .collect()
    }

    fn query_region_data(&self, region: &RegionCoords) -> D {
        self.query_op_data(region)
            .into_iter()
            .map(|o| o.region_data())
            .fold(D::zero(), |d, o| d + o)
    }

    fn fetch_region_set(
        &self,
        coords: crate::prelude::RegionCoordSetLtcs,
    ) -> must_future::MustBoxFuture<Result<crate::prelude::RegionSetLtcs<D>, ()>> {
        async move { coords.into_region_set(|(_, coords)| Ok(self.query_region_data(&coords))) }
            .boxed()
            .into()
    }

    fn integrate_ops<Ops: Clone + Iterator<Item = Arc<O>>>(&mut self, ops: Ops) {
        // for op in ops.clone() {
        //     self.region_set.add(op.region_tuple(self.region_set.topo()));
        // }
        self.ops.extend(ops);
    }

    fn topo(&self) -> &Topology {
        &self.topo
    }

    fn gossip_params(&self) -> GossipParams {
        self.gossip_params
    }
}

// impl OpStore<RegionData> {
//     fn integrate_ops<O: Iterator<Item = Op>>(&mut self, ops: O) {
//         for op in ops {
//             self.tree.add_op(op);
//         }
//         self.ops.extend(ops);
//     }
// }

// fn op_bound(timestamp: Timestamp, loc: Loc) -> OpData {
//     OpData {
//         loc,
//         timestamp,
//         size: 0,
//         hash: [0; 32].into(),
//     }
// }
