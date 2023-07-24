use crate::sweettest::sweet_topos::graph::NetworkTopologyGraph;
use crate::sweettest::sweet_topos::node::NetworkTopologyNode;
use contrafact::Fact;
use contrafact::Generator;
use contrafact::Mutation;
use std::ops::RangeInclusive;

/// Fact:
/// - The network has a specific number of nodes.
/// - Each node has a specific number of agents.
struct SizedNetworkFact {
    /// The number of nodes in the network.
    /// Ideally this would be a range, but we can't do that yet.
    nodes: usize,
}

impl SizedNetworkFact {
    pub fn new(nodes: usize) -> Self {
        Self { nodes }
    }

    pub fn from_range(g: &mut Generator, nodes: RangeInclusive<usize>) -> Mutation<Self> {
        Ok(Self {
            nodes: g.int_in_range(nodes, "Couldn't build a fact in the range.")?,
        })
    }
}

impl<'a> Fact<'a, NetworkTopologyGraph> for SizedNetworkFact {
    fn mutate(
        &self,
        mut graph: NetworkTopologyGraph,
        g: &mut Generator<'a>,
    ) -> Mutation<NetworkTopologyGraph> {
        let mut node_count = graph.node_count();
        while node_count < self.nodes {
            let node = g.arbitrary::<NetworkTopologyNode>("Could not create node")?;
            graph.add_node(node);
            node_count = graph.node_count();
        }
        while node_count > self.nodes {
            graph.remove_node(
                g.int_in_range(0..=node_count, "could not remove node")?
                    .into(),
            );
            node_count = graph.node_count();
        }
        Ok(graph)
    }

    /// Not sure what a meaningful advance would be as a graph is already a
    /// collection, so why would we want a sequence of them?
    fn advance(&mut self, _graph: &NetworkTopologyGraph) {
        todo!();
    }
}

#[cfg(test)]
pub mod test {
    /// Test that we can build a network with zero nodes.
    #[test]
    fn test_sweet_topos_sized_network_zero_nodes() {
        let mut g = unstructured_noise().into();
        let mut fact = SizedNetworkFact { nodes: 0 };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
        assert_eq!(graph.strict_partitions(), 0);
    }

    /// Test that we can build a network with a single node.
    #[test]
    fn test_sweet_topos_sized_network_single_node() {
        let mut g = unstructured_noise().into();
        let mut fact = SizedNetworkFact { nodes: 1 };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
        assert_eq!(graph.strict_partitions(), 1);
    }

    /// Test that we can build a network with a dozen nodes.
    #[test]
    fn test_sweet_topos_sized_network_dozen_nodes() {
        let mut g = unstructured_noise().into();
        let mut fact = SizedNetworkFact { nodes: 12 };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert_eq!(graph.node_count(), 12);
        assert_eq!(graph.edge_count(), 0);
        assert_eq!(graph.strict_partitions(), 12);
    }

    /// Test that we can build a network with a number of nodes within a range.
    #[test]
    fn test_sweet_topos_sized_network_range() {
        let mut g = unstructured_noise().into();
        let mut fact = SizedNetworkFact::from_range(&mut g, 1..=10).unwrap();
        let graph = fact.build_fallible(&mut g).unwrap();
        assert!(graph.node_count() >= 1);
        assert!(graph.node_count() <= 10);
        assert_eq!(graph.node_count(), fact.nodes);
        assert_eq!(graph.edge_count(), 0);
        assert_eq!(graph.strict_partitions(), fact.nodes);
    }
}
