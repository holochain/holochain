use arbitrary::Arbitrary;
use holochain_zome_types::prelude::CellId;

/// A node in a network topology. Represents a conductor.
#[derive(Arbitrary, Clone, Debug, PartialEq, Default)]
pub struct NetworkTopologyNode {
    cells: Vec<CellId>,
}

impl NetworkTopologyNode {
    /// Create a new node with the given cells.
    pub fn new(cells: Vec<CellId>) -> Self {
        Self { cells }
    }

    /// Get the cells in this node.
    pub fn cells(&self) -> &Vec<CellId> {
        &self.cells
    }
}
