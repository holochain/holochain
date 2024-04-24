use crate::sweettest::sweet_topos::network::NetworkTopology;
use crate::sweettest::sweet_topos::node::NetworkTopologyNode;
use contrafact::Fact;
use contrafact::Generator;
use contrafact::Mutation;
use holochain_util::tokio_helper;
use std::ops::RangeInclusive;

/// Fact:
/// - The network has a specific number of nodes.
/// - Each node has a specific number of agents.
#[derive(Clone, Debug, PartialEq)]
pub struct SizedNetworkFact {
    /// The number of nodes in the network.
    /// Ideally this would be a range, but we can't do that yet.
    pub nodes: usize,
    /// The number of agents in each node.
    pub agents: RangeInclusive<usize>,
}

impl SizedNetworkFact {
    /// Create a new fact with a number of nodes in the given range.
    pub fn from_range(
        g: &mut Generator,
        nodes: RangeInclusive<usize>,
        agents: RangeInclusive<usize>,
    ) -> Mutation<Self> {
        Ok(Self {
            nodes: g.int_in_range(nodes, || "Couldn't build a fact in the range.")?,
            agents,
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
            let mut node = NetworkTopologyNode::new();
            node.ensure_dnas(network_topology.dnas().to_vec());
            let n = g.int_in_range(self.agents.clone(), || "could not generate cells")?;
            tokio_helper::block_forever_on(async { node.generate_cells(n).await });
            network_topology.add_node(node);
            node_count = network_topology.node_count();
        }
        while node_count > self.nodes {
            network_topology
                .remove_node_index(g.int_in_range(0..=node_count, || "could not remove node")?);
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
    use crate::prelude::unstructured_noise;

    /// Test that we can build a network with zero nodes.
    #[test]
    fn test_sweet_topos_sized_network_zero_nodes() {
        let mut g = unstructured_noise().into();
        let fact = SizedNetworkFact {
            nodes: 0,
            agents: 1..=1,
        };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
        assert_eq!(graph.strict_partitions(), 0);
    }

    /// Test that we can build a network with a single node.
    #[test]
    fn test_sweet_topos_sized_network_single_node() {
        crate::big_stack_test!(
            async move {
                let mut g = unstructured_noise().into();
                let fact = SizedNetworkFact {
                    nodes: 1,
                    agents: 1..=1,
                };
                let graph = fact.build_fallible(&mut g).unwrap();
                assert_eq!(graph.node_count(), 1);
                assert_eq!(graph.edge_count(), 0);
                assert_eq!(graph.strict_partitions(), 1);
            },
            7_000_000
        );
    }

    /// Test that we can build a network with a dozen nodes.
    #[test]
    fn test_sweet_topos_sized_network_dozen_nodes() {
        crate::big_stack_test!(
            async move {
                let mut g = unstructured_noise().into();
                let fact = SizedNetworkFact {
                    nodes: 12,
                    agents: 1..=2,
                };
                let graph = fact.build_fallible(&mut g).unwrap();
                assert_eq!(graph.node_count(), 12);
                assert_eq!(graph.edge_count(), 0);
                assert_eq!(graph.strict_partitions(), 12);
            },
            7_000_000
        );
    }

    /// Test that we can build a network with a number of nodes within a range.
    #[test]
    fn test_sweet_topos_sized_network_range() {
        crate::big_stack_test!(
            async move {
                let mut g = unstructured_noise().into();
                let fact = SizedNetworkFact::from_range(&mut g, 1..=10, 3..=5).unwrap();
                let graph = fact.clone().build_fallible(&mut g).unwrap();
                assert!(graph.node_count() >= 1);
                assert!(graph.node_count() <= 10);
                assert_eq!(graph.node_count(), fact.nodes);
                assert_eq!(graph.edge_count(), 0);
                assert_eq!(graph.strict_partitions(), fact.nodes);
            },
            7_000_000
        )
    }
}
