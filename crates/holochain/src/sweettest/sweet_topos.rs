use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use contrafact::*;
use derive_more::DerefMut;
use petgraph::algo::connected_components;
use petgraph::dot::{Config, Dot};
use petgraph::prelude::*;
use petgraph::unionfind::UnionFind;
use petgraph::visit::NodeIndexable;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use shrinkwraprs::Shrinkwrap;
use std::ops::RangeInclusive;

#[derive(Arbitrary, Clone, Debug, PartialEq, Default)]
struct NetworkTopologyNode;
#[derive(Arbitrary, Clone, Debug, PartialEq, Default)]
struct NetworkTopologyEdge;

#[derive(Clone, Debug, Shrinkwrap, Default, DerefMut)]
struct NetworkTopologyGraph(Graph<NetworkTopologyNode, NetworkTopologyEdge, Directed, usize>);

/// Implement arbitrary for NetworkTopologyGraph by simply iterating over some
/// arbitrary nodes and edges and adding them to the graph. This allows self
/// edges and duplicate edges, but that's fine for our purposes as it will simply
/// cause agent info to be added multiple times or to the same agent.
impl<'a> Arbitrary<'a> for NetworkTopologyGraph {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut graph = Graph::default();

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
            graph.add_node(NetworkTopologyNode);
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

struct StrictlyPartitionedNetworkFact {
    partitions: usize,
}

impl<'a> Fact<'a, NetworkTopologyGraph> for StrictlyPartitionedNetworkFact {
    fn mutate(
        &self,
        mut graph: NetworkTopologyGraph,
        g: &mut Generator<'a>,
    ) -> Mutation<NetworkTopologyGraph> {
        // Remove edges until the graph is partitioned into the desired number of
        // partitions. The edges are removed randomly, so this is not the most
        // efficient way to do this, but it's simple and it works.
        while connected_components(graph.as_ref()) < self.partitions {
            dbg!("removing edge");
            let edge_indices = graph.edge_indices().collect::<Vec<_>>();
            let max_edge_index = graph.edge_count() - 1;
            graph.remove_edge(
                edge_indices
                    .iter()
                    .nth(
                        g.int_in_range(0..=max_edge_index, "could not select an edge to remove")?
                            .into(),
                    )
                    .ok_or(MutationError::Exception(
                        "could not select an edge to remove".to_string(),
                    ))?
                    .clone(),
            );
        }

        // Add edges until the graph is connected up to the desired number of
        // partitions.
        while connected_components(graph.as_ref()) > self.partitions {
            dbg!("adding edge");
            // Taken from `connected_components` in petgraph.
            let mut vertex_sets = UnionFind::new(graph.node_bound());
            for edge in graph.edge_references() {
                let (a, b) = (edge.source(), edge.target());

                // union the two vertices of the edge
                vertex_sets.union(graph.to_index(a), graph.to_index(b));
            }

            // Pick a random node from the graph.
            let node_index = graph
                .node_indices()
                .nth(
                    g.int_in_range(
                        0..=(graph.node_count() - 1),
                        "could not select a node to connect",
                    )?
                    .into(),
                )
                .ok_or(MutationError::Exception(
                    "could not select a node to connect".to_string(),
                ))?;

            // Iterate over all the other nodes in the graph, shuffled. For each
            // node, if it's not already connected to the node we picked, add an
            // edge between them and break out of the loop. The RNG is seeded
            // by the generator, so this should be deterministic per generator.
            let seed: [u8; 32] = g
                .bytes(32)?
                .try_into()
                .map_err(|_| MutationError::Exception("failed to seed the rng".into()))?;
            let mut rng = rand_chacha::ChaCha20Rng::from_seed(seed);
            let mut other_node_indexes = graph.node_indices().collect::<Vec<_>>();
            other_node_indexes.shuffle(&mut rng);

            for other_node_index in other_node_indexes {
                if vertex_sets.find(node_index.index())
                    != vertex_sets.find(other_node_index.index())
                {
                    graph.add_edge(node_index, other_node_index, NetworkTopologyEdge);
                    break;
                }
            }
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
        let mut fact = SizedNetworkFact { nodes: 0 };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert_eq!(graph.node_count(), 0);
    }

    /// Test that we can build a network with a single node.
    #[test]
    fn test_sweet_topos_sized_network_single_node() {
        let mut g = unstructured_noise().into();
        let mut fact = SizedNetworkFact { nodes: 1 };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert_eq!(graph.node_count(), 1);
    }

    /// Test that we can build a network with a dozen nodes.
    #[test]
    fn test_sweet_topos_sized_network_dozen_nodes() {
        let mut g = unstructured_noise().into();
        let mut fact = SizedNetworkFact { nodes: 12 };
        let graph = fact.build_fallible(&mut g).unwrap();
        assert_eq!(graph.node_count(), 12);
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
    }

    /// Test that we can build a network with one partition.
    #[test]
    fn test_sweet_topos_strictly_partitioned_network_one_partition() {
        let mut g = unstructured_noise().into();
        let size_fact = SizedNetworkFact { nodes: 3 };
        let partition_fact = StrictlyPartitionedNetworkFact { partitions: 1 };
        let facts = facts![size_fact, partition_fact];
        let mut graph = NetworkTopologyGraph::default();
        graph = facts.mutate(graph, &mut g).unwrap();
        assert_eq!(connected_components(graph.as_ref()), 1);

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
    }

    /// Test that we can build a network with a dozen nodes and three partitions.
    #[test]
    fn test_sweet_topos_strictly_partitioned_network_dozen_nodes_three_partitions() {
        let mut g = unstructured_noise().into();
        let size_fact = SizedNetworkFact { nodes: 12 };
        let partition_fact = StrictlyPartitionedNetworkFact { partitions: 3 };
        let facts = facts![size_fact, partition_fact];
        let mut graph = NetworkTopologyGraph::default();
        graph = facts.mutate(graph, &mut g).unwrap();
        assert_eq!(connected_components(graph.as_ref()), 3);

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
    }
}
