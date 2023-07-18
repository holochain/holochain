use arbitrary::Arbitrary;
use holochain_zome_types::prelude::CellId;
use super::node::NetworkTopologyNode;

#[derive(Arbitrary, Clone, Debug, PartialEq, Default)]
pub struct NetworkTopologyEdge{
    cells: Vec<CellId>,
}

impl NetworkTopologyEdge {
    pub fn new(cells: Vec<CellId>) -> Self {
        Self { cells }
    }

    pub fn new_full_view_on_node(node: &NetworkTopologyNode) -> Self {
        Self {
            cells: node.cells().clone(),
        }
    }
}