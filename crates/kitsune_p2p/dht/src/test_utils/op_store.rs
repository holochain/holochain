use std::{collections::BTreeSet, ops::Bound, sync::Arc};

use crate::{
    coords::{GossipParams, Topology},
    host::AccessOpStore,
    op::{OpData, OpRegion, Timestamp},
    region::{RegionBounds, RegionData},
    tree::{Tree, TreeDataConstraints},
    Loc,
};

#[derive(Clone)]
pub struct OpStore<D: TreeDataConstraints = RegionData, O: OpRegion<D> = OpData> {
    pub(crate) ops: BTreeSet<Arc<O>>,
    pub(crate) tree: Tree<D>,
    pub(crate) gossip_params: GossipParams,
}

impl<D: TreeDataConstraints, O: OpRegion<D>> OpStore<D, O> {
    pub fn new(topo: Topology, gossip_params: GossipParams) -> Self {
        Self {
            ops: Default::default(),
            tree: Tree::new(topo),
            gossip_params,
        }
    }
}

impl<D: TreeDataConstraints, O: OpRegion<D>> AccessOpStore<D, O> for OpStore<D, O> {
    fn query_op_data(&self, region: &RegionBounds) -> Vec<Arc<O>> {
        let (x0, x1) = region.x;
        let (t0, t1) = region.t;
        let op0 = O::bound(Timestamp::from_micros(*t0 as i64), Loc::from(*x0));
        let op1 = O::bound(Timestamp::from_micros(*t1 as i64), Loc::from(*x1));
        self.ops
            .range((Bound::Included(op0), Bound::Included(op1)))
            .filter(|o| *x0 <= o.loc().as_u32() && o.loc().as_u32() <= *x1)
            .cloned()
            .collect()
    }

    fn query_region_data(&self, region: &RegionBounds) -> D {
        self.tree.lookup(region)
    }

    fn integrate_ops<Ops: Clone + Iterator<Item = Arc<O>>>(&mut self, ops: Ops) {
        for op in ops.clone() {
            self.tree.add(op.region_tuple(self.tree.topo()));
        }
        self.ops.extend(ops);
    }

    fn topo(&self) -> &Topology {
        self.tree.topo()
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
