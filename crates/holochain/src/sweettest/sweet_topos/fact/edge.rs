use crate::sweettest::sweet_topos::edge::NetworkTopologyEdge;
use crate::sweettest::sweet_topos::node::NetworkTopologyNode;
use contrafact::Fact;
use contrafact::Generator;
use contrafact::Mutation;

/// Fact: The origin node can see every agent on the target node.
pub struct FullAgentViewFact {
    /// The target node. This node is fully visible to the origin node.
    target: NetworkTopologyNode,
}

impl<'a> Fact<'a, NetworkTopologyEdge> for FullAgentViewFact {
    fn mutate(
        &self,
        mut edge: NetworkTopologyEdge,
        g: &mut Generator<'a>,
    ) -> Mutation<NetworkTopologyEdge> {
        edge = NetworkTopologyEdge::new_full_view_on_node(&self.target);
        Ok(edge)
    }

    fn advance(&mut self, _edge: &NetworkTopologyEdge) {
        todo!();
    }
}
