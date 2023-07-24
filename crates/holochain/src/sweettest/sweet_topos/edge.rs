use super::node::NetworkTopologyNode;
use arbitrary::Arbitrary;
use holochain_zome_types::prelude::CellId;

/// A network edge in a network topology. Represents a network connection.
/// Edges are directed, so if you want a bidirectional connection you need two
/// edges.
#[derive(Arbitrary, Clone, Debug, PartialEq, Default)]
pub struct NetworkTopologyEdge {
    cells: Vec<CellId>,
}

impl NetworkTopologyEdge {
    /// Create a new edge with the given cells.
    pub fn new(cells: Vec<CellId>) -> Self {
        Self { cells }
    }

    /// Create a new edge with a full view on the given node.
    pub fn new_full_view_on_node(node: &NetworkTopologyNode) -> Self {
        Self {
            cells: node.cells().clone(),
        }
    }
}
