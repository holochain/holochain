use crate::sweettest::sweet_topos::network::NetworkTopology;
use crate::sweettest::sweet_topos::node::NetworkTopologyNode;
use contrafact::Fact;
use contrafact::Generator;
use contrafact::Mutation;
use std::ops::RangeInclusive;

/// Fact:
/// - The network has a specific number of nodes.
/// - Each node has a specific number of agents.
#[derive(Clone, Debug)]
pub struct SizedNetworkFact {
    /// The number of nodes in the network.
    /// Ideally this would be a range, but we can't do that yet.
    nodes: usize,
}

impl SizedNetworkFact {
    /// Create a new fact with the given number of nodes.
    pub fn new(nodes: usize) -> Self {
        Self { nodes }
    }

    /// Create a new fact with a number of nodes in the given range.
    pub fn from_range(g: &mut Generator, nodes: RangeInclusive<usize>) -> Mutation<Self> {
        Ok(Self {
            nodes: g.int_in_range(nodes, || "Couldn't build a fact in the range.")?,
        })
    }
}

impl<'a> Fact<'a, NetworkTopology> for SizedNetworkFact {
    fn mutate(
        &mut self,
        g: &mut Generator<'a>,
        mut network_topology: NetworkTopology,
    ) -> Mutation<NetworkTopology> {
        let mut node_count = network_topology.node_count();
        while node_count < self.nodes {
            let node: NetworkTopologyNode = g.arbitrary(|| "Could not create node")?;
            network_topology.add_node(node);
            node_count = network_topology.node_count();
        }
        while node_count > self.nodes {
            network_topology.remove_node_index(
                g.int_in_range(0..=node_count, || "could not remove node")?
                    .into(),
            );
            node_count = network_topology.node_count();
        }
        Ok(network_topology)
    }

    fn label(&self) -> String {
        todo!()
    }

    fn labeled(self, _label: impl ToString) -> Self {
        todo!()
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    /// Test that we can build a sized network fact with `SizedNetworkFact::new`.
    #[test]
    fn test_sized_network_fact_new() {
        let a = SizedNetworkFact::new(3);
        let b = SizedNetworkFact { nodes: 3 };
        assert_eq!(a, b);
    }

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
