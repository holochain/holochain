use std::collections::BinaryHeap;

use crate::{region::Region, region::RegionData, tree::Tree};

#[derive(derive_more::Deref)]
pub struct HeapRegion(Region);

impl PartialEq for HeapRegion {
    fn eq(&self, other: &Self) -> bool {
        self.0.data.size == other.0.data.size
    }
}

impl Eq for HeapRegion {}

impl PartialOrd for HeapRegion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.data.size.partial_cmp(&other.0.data.size)
    }
}

impl Ord for HeapRegion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.data.size.partial_cmp(&other.0.data.size).unwrap()
    }
}

pub struct Partition {
    heap: BinaryHeap<HeapRegion>,
}

impl Partition {
    pub fn optimize(&mut self, tree: &Tree) {
        while self.overhead() < self.weight(tree) {
            let r = self.heap.pop().unwrap();
            let (r1, r2) = r.0.split(tree).expect("Can't split the leaves");
            self.heap.push(HeapRegion(r1));
            self.heap.push(HeapRegion(r2));
        }
    }

    fn overhead(&self) -> usize {
        self.heap.iter().count() * RegionData::MASS
    }

    fn weight(&self, tree: &Tree) -> usize {
        // TODO: optimize
        self.heap
            .iter()
            .map(|r| tree.lookup(&r.coords.to_bounds()).size as usize)
            .sum()
    }
}
