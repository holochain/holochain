use sparse_fenwick::Fenwick2;

use crate::{
    coords::{RegionCoords, Topology},
    op::Op,
    region_data::RegionData,
};

pub struct Tree {
    tree: Fenwick2<RegionData>,
    q: Topology,
}

impl Tree {
    pub fn add_op(&mut self, op: Op) {
        let (coords, data) = op.to_node(&self.q);
        self.tree.update(coords.to_tuple(), data);
    }

    pub fn lookup(&self, region: &RegionCoords) -> RegionData {
        let (sa, sb) = region.space.bounds();
        let (ta, tb) = region.time.bounds();
        self.tree.query((sa, ta), (sb, tb))
    }
}
