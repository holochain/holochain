use crate::sweettest::sweet_topos::edge::NetworkTopologyEdge;
use crate::sweettest::sweet_topos::node::NetworkTopologyNode;
use contrafact::Fact;
use contrafact::Generator;
use contrafact::Mutation;

/// Fact: The origin node can see every agent on the target node.
#[derive(Clone, Debug)]
pub struct FullAgentViewFact {
    /// The source node. This node can see every agent on the target node.
    source: NetworkTopologyNode,
    /// The target node. This node is fully visible to the origin node.
    target: NetworkTopologyNode,
}

impl<'a> Fact<'a, NetworkTopologyEdge> for FullAgentViewFact {
    fn mutate(
        &mut self,
        _g: &mut Generator<'a>,
        mut _edge: NetworkTopologyEdge,
    ) -> Mutation<NetworkTopologyEdge> {
        let new_edge = NetworkTopologyEdge::new_full_view_on_node(&self.source, &self.target);
        Ok(new_edge)
    }

    fn label(&self) -> String {
        todo!()
    }

    fn labeled(self, _: impl ToString) -> Self {
        todo!()
    }
}
