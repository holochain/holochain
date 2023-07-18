use arbitrary::Arbitrary;
use holochain_zome_types::prelude::CellId;

#[derive(Arbitrary, Clone, Debug, PartialEq, Default)]
pub struct NetworkTopologyNode {
    cells: Vec<CellId>,
}

impl NetworkTopologyNode {
    pub fn new(cells: Vec<CellId>) -> Self {
        Self { cells }
    }

    pub fn cells(&self) -> &Vec<CellId> {
        &self.cells
    }
}