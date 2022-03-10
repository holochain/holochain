use crate::{
    op::{OpData, OpRegion},
    persistence::AccessOpStore,
    prelude::{RegionSet, RegionSetXtcs},
    quantum::{GossipParams, Topology},
    region::{RegionBounds, RegionData},
    tree::TreeDataConstraints,
};
use futures::future::FutureExt;
use std::{collections::BTreeSet, ops::Bound, sync::Arc};

#[derive(Clone)]
pub struct OpStore<D: TreeDataConstraints = RegionData, O: OpRegion<D> = OpData> {
    pub(crate) topo: Topology,
    pub(crate) ops: BTreeSet<Arc<O>>,
    pub(crate) _region_set: RegionSet<D>,
    pub(crate) gossip_params: GossipParams,
}

impl<D: TreeDataConstraints, O: OpRegion<D>> OpStore<D, O> {
    pub fn new(topo: Topology, gossip_params: GossipParams) -> Self {
        Self {
            topo,
            ops: Default::default(),
            _region_set: RegionSetXtcs::empty().into(),
            gossip_params,
        }
    }
}

impl<D: TreeDataConstraints, O: OpRegion<D>> AccessOpStore<D, O> for OpStore<D, O> {
    fn query_op_data(&self, region: &RegionBounds) -> Vec<Arc<O>> {
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

    fn query_region(&self, region: &RegionBounds) -> D {
        self.query_op_data(region)
            .into_iter()
            .map(|o| o.region_data())
            .fold(D::zero(), |d, o| d + o)
    }

    fn fetch_region_set(
        &self,
        coords: crate::prelude::RegionCoordSetXtcs,
    ) -> must_future::MustBoxFuture<Result<crate::prelude::RegionSetXtcs<D>, ()>> {
        async move { coords.into_region_set(|(_, coords)| Ok(self.query_region_coords(&coords))) }
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
        self.gossip_params.clone()
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
