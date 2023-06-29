use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use contrafact::*;
use derive_more::DerefMut;
use petgraph::dot::{Config, Dot};
use petgraph::prelude::*;
use shrinkwraprs::Shrinkwrap;
use std::ops::RangeInclusive;

#[derive(Arbitrary, Clone, Debug, PartialEq, Default)]
struct NetworkTopologyNode;
#[derive(Arbitrary, Clone, Debug, PartialEq, Default)]
struct NetworkTopologyEdge;

#[derive(Clone, Debug, Shrinkwrap, Default, DerefMut)]
struct NetworkTopologyGraph(StableGraph<NetworkTopologyNode, NetworkTopologyEdge, Directed, usize>);

/// Implement arbitrary for NetworkTopologyGraph by simply iterating over some
/// arbitrary nodes and edges and adding them to the graph. This allows self
/// edges and duplicate edges, but that's fine for our purposes as it will simply
/// cause agent info to be added multiple times or to the same agent.
impl<'a> Arbitrary<'a> for NetworkTopologyGraph {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut graph = StableGraph::default();

        // Get an iterator of arbitrary `NetworkTopologyNode`s.
        let nodes = u.arbitrary_iter::<NetworkTopologyNode>()?;
        for node in nodes {
            graph.add_node(node?);
        }

        if graph.node_count() > 0 {
            // Add some edges.
            let edges: arbitrary::Result<Vec<NetworkTopologyEdge>> =
                u.arbitrary_iter::<NetworkTopologyEdge>()?.collect();
            for edge in edges? {
                let max_node_index = graph.node_count() - 1;
                let a = u.int_in_range(0..=max_node_index)?.into();
                let b = u.int_in_range(0..=max_node_index)?.into();
                graph.add_edge(a, b, edge);
            }
        }

        Ok(Self(graph))
    }
}

impl PartialEq for NetworkTopologyGraph {
    fn eq(&self, other: &Self) -> bool {
        // This is a bit of a hack, but hopefully it works.
        format!(
            "{:?}",
            Dot::with_config(
                self.as_ref(),
                &[
                    Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        ) == format!(
            "{:?}",
            Dot::with_config(
                other.as_ref(),
                &[
                    Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        )
    }
}

struct SizedNetworkFact {
    nodes: RangeInclusive<usize>,
}

impl<'a> Fact<'a, NetworkTopologyGraph> for SizedNetworkFact {
    fn mutate(
        &self,
        mut graph: NetworkTopologyGraph,
        g: &mut Generator<'a>,
    ) -> Mutation<NetworkTopologyGraph> {
        dbg!("mutate");
        dbg!(&graph);
        let desired_node_count = g.int_in_range(
            self.nodes.clone(),
            "could not calculate a desired node count",
        )?;
        let mut node_count = graph.node_count();
        while node_count < desired_node_count {
            dbg!("add");
            dbg!(node_count);
            dbg!(g.len());
            graph.add_node(NetworkTopologyNode);
            node_count = graph.node_count();
        }
        while node_count > desired_node_count {
            dbg!("remove");
            dbg!(node_count);
            dbg!(g.len());
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

// struct StrictlyPartitionedNetworkFact {
//     max_partitions: usize,
// }

// impl<'a> Fact<'a, NetworkTopologyGraph> for Brute

// impl<'a> Fact<'a, NetworkTopologyGraph> for StrictlyPartitionedNetworkFact {
//     fn mutate(&self, mut graph: NetworkTopologyGraph), g: &mut Generator<'a>) -> Mutation<NetworkTopologyGraph> {
//         if graph.node_count() < self.min_partitions
//     }
// }

#[cfg(test)]
pub mod test {
    use super::*;
    use holochain_zome_types::entropy::unstructured_noise;

    /// Test the arbitrary implementation for NetworkTopologyGraph.
    #[test]
    fn test_sweet_topos_arbitrary() -> anyhow::Result<()> {
        let mut u = unstructured_noise();
        let graph = NetworkTopologyGraph::arbitrary(&mut u)?;
        // It's arbitrary, so we can't really assert anything about it, but we
        // can print it out to see what it looks like.
        println!(
            "{:?}",
            Dot::with_config(
                graph.as_ref(),
                &[
                    Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        );
        Ok(())
    }

    /// Test that we can build a network with zero nodes.
    #[test]
    fn test_sweet_topos_sized_network_zero_nodes() {
        let mut g = unstructured_noise().into();
        let mut fact = SizedNetworkFact { nodes: 0..=0 };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert_eq!(graph.node_count(), 0);
    }

    /// Test that we can build a network with a single node.
    #[test]
    fn test_sweet_topos_sized_network_single_node() {
        let mut g = unstructured_noise().into();
        let mut fact = SizedNetworkFact { nodes: 1..=1 };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert_eq!(graph.node_count(), 1);
    }

    /// Test that we can build a network with a number of nodes within a range.
    #[test]
    fn test_sweet_topos_sized_network_range() {
        let mut g = unstructured_noise().into();
        let mut fact = SizedNetworkFact { nodes: 1..=10 };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert!(graph.node_count() >= 1);
        assert!(graph.node_count() <= 10);
    }
}
