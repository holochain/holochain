use sparse_fenwick::Fenwick2;

use crate::{
    coords::{Coord, RegionBounds, SpaceCoord, TimeCoord, Topology},
    op::Op,
    region_data::RegionData,
};

pub struct Tree {
    tree: Fenwick2<RegionData>,
    topo: Topology,
}

impl Tree {
    pub fn new(topo: Topology) -> Self {
        Self {
            tree: Fenwick2::new((SpaceCoord::MAX, TimeCoord::MAX)),
            topo,
        }
    }

    pub fn add_op(&mut self, op: Op) {
        let (coords, data) = op.to_tree_data(&self.topo);
        self.tree.update(coords.to_tuple(), data);
    }

    pub fn lookup(&self, region: &RegionBounds) -> RegionData {
        let (sa, sb) = region.x;
        let (ta, tb) = region.t;
        self.tree.query((*sa, *ta), (*sb, *tb))
    }
}
