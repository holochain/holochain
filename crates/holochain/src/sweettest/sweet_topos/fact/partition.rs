use contrafact::Fact;
use contrafact::Generator;
use contrafact::Mutation;
use rand::Rng;
// use contrafact::MutationError;
// use petgraph::unionfind::UnionFind;
// use rand::prelude::SliceRandom;
// use std::collections::HashMap;
// use rand::SeedableRng;
// use crate::sweettest::fact::edge::FullAgentViewFact;
use crate::sweettest::sweet_topos::graph::NetworkTopologyGraph;
// use petgraph::prelude::*;

/// Fact:
/// - The network has a specific number of partitions.
/// - The network is partitioned as strictly as possible. This means that there
///  are no edges between nodes in different partitions.
/// - The partition generation process has a specific efficiency. More efficient
/// partitioning means that the partitions heal more quickly which can lead to
/// one or a few partitions dominating the network. Less efficient partitioning
/// means that the partitions heal more slowly which can lead to a more even
/// distribution of partitions.
struct StrictlyPartitionedNetworkFact {
    /// The number of partitions in the network.
    partitions: usize,
    /// The efficiency of the partitioning process. This is a number between 0
    /// and 1. The higher the number, the more efficient the partitioning
    /// process.
    efficiency: f64,
}

impl<'a> Fact<'a, NetworkTopologyGraph> for StrictlyPartitionedNetworkFact {
    fn mutate(
        &self,
        mut graph: NetworkTopologyGraph,
        g: &mut Generator<'a>,
    ) -> Mutation<NetworkTopologyGraph> {
        let mut rng: _ = super::rng_from_generator(g);
        let efficiency_cutoff = (self.efficiency * u64::MAX as f64) as u64;

        // Remove edges until the graph is partitioned into the desired number of
        // partitions. The edges are removed randomly, so this is not the most
        // efficient way to do this, but it's simple and it works.
        while graph.strict_partitions() < self.partitions {
            graph.remove_random_edge(&mut rng)?;
            // let edge_indices = graph.edge_indices().collect::<Vec<_>>();
            // let max_edge_index = graph.edge_count() - 1;
            // graph.remove_edge(
            //     edge_indices
            //         .iter()
            //         .nth(
            //             g.int_in_range(0..=max_edge_index, "could not select an edge to remove")?
            //                 .into(),
            //         )
            //         .ok_or(MutationError::Exception(
            //             "could not select an edge to remove".to_string(),
            //         ))?
            //         .clone(),
            // );
        }

        // Add edges until the graph is connected up to the desired number of
        // partitions.
        while graph.strict_partitions() > self.partitions {
            // // Taken from `connected_components` in petgraph.
            // // Builds our view on the partitions as they are.
            // let mut vertex_sets = UnionFind::new(graph.node_bound());
            // for edge in graph.edge_references() {
            //     let (a, b) = (edge.source(), edge.target());

            //     // union the two vertices of the edge
            //     vertex_sets.union(graph.to_index(a), graph.to_index(b));
            // }

            // // Pick a random node from the graph.
            // let node_index = graph
            //     .node_indices()
            //     .nth(
            //         g.int_in_range(
            //             0..=(graph.node_count() - 1),
            //             "could not select a node to connect",
            //         )?
            //         .into(),
            //     )
            //     .ok_or(MutationError::Exception(
            //         "could not select a node to connect".to_string(),
            //     ))?;

            let efficiency_switch = rng.gen::<u64>();

            // let efficiency_switch =
            //     u64::from_le_bytes(g.bytes(std::mem::size_of::<u64>())?.try_into().map_err(
            //         |_| MutationError::Exception("failed to build bytes for int".into()),
            //     )?);
            //     // The RNG is seeded
            //     // by the generator, so this should be deterministic per generator.
            //     let seed: [u8; 32] = g
            //     .bytes(32)?
            //     .try_into()
            //     .map_err(|_| MutationError::Exception("failed to seed the rng".into()))?;
            // let mut rng = rand_chacha::ChaCha20Rng::from_seed(seed);

            // If the efficiency switch is above the cutoff, we'll reassign an
            // existing node to a different partition. Otherwise, we'll add a new
            // edge between two nodes in different partitions.
            // We can't reassign a node to a different partition if there's only
            // one desired partition, so we'll just add an edge in that case.
            if efficiency_switch > efficiency_cutoff && self.partitions > 1 {
                graph.reassign_random_node_to_smallest_strict_partition(&mut rng)?;

                // let labels = vertex_sets.clone().into_labeling();
                // let mut m: HashMap<usize, usize> = HashMap::new();
                // for label in &labels {
                //     *m.entry(*label).or_default() += 1;
                // }
                // let representative_of_smallest_partition = m
                //     .into_iter()
                //     .min_by_key(|(_, v)| *v)
                //     .map(|(k, _)| k)
                //     .ok_or(MutationError::Exception(
                //         "could not find smallest partition".to_string(),
                //     ))?;

                // let mut nodes_in_smallest_partition = labels
                //     .iter()
                //     .enumerate()
                //     .filter(|(i, label)| **label == representative_of_smallest_partition)
                //     .map(|(i, _)| i)
                //     .collect::<Vec<_>>();
                // nodes_in_smallest_partition.shuffle(&mut rng);

                // while let Some(edge) = graph.first_edge(node_index, Direction::Outgoing) {
                //     graph.remove_edge(edge);
                // }
                // while let Some(edge) = graph.first_edge(node_index, Direction::Incoming) {
                //     graph.remove_edge(edge);
                // }
                // let other_node_index = NodeIndex::from(
                //     *nodes_in_smallest_partition
                //         .iter()
                //         .next()
                //         .ok_or::<MutationError>(
                //             MutationError::Exception(
                //                 "There were no nodes in the smallest partition".to_string(),
                //             )
                //             .into(),
                //         )?,
                // );

                // // If the node in the smallest partition is the node we picked,
                // // do nothing this round.
                // if node_index != other_node_index {
                //     let edge = FullAgentViewFact {
                //         origin: node_index,
                //         target: other_node_index,
                //     }
                //     .build_fallible(g)?;
                //     graph.add_edge(node_index, other_node_index, edge);
                // }
            } else {
                graph.heal_random_strict_partition(&mut rng)?;

                // Iterate over all the other nodes in the graph, shuffled. For each
                // node, if it's not already connected to the node we picked, add an
                // edge between them and break out of the loop.

                // let mut other_node_indexes = graph.node_indices().collect::<Vec<_>>();
                // other_node_indexes.shuffle(&mut rng);

                // for other_node_index in other_node_indexes {
                //     if vertex_sets.find(node_index.index())
                //         != vertex_sets.find(other_node_index.index())
                //     {
                //         let edge = FullAgentViewFact {
                //             origin: node_index,
                //             target: other_node_index,
                //         }
                //         .build_fallible(g)?;
                //         graph.add_edge(node_index, other_node_index, edge);
                //         break;
                //     }
                // }
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
    /// Test that we can build a network with one partition.
    #[test]
    fn test_sweet_topos_strictly_partitioned_network_one_partition() {
        let mut g = unstructured_noise().into();
        let size_fact = SizedNetworkFact { nodes: 3 };
        let partition_fact = StrictlyPartitionedNetworkFact {
            partitions: 1,
            efficiency: 1.0,
        };
        let facts = facts![size_fact, partition_fact];
        let mut graph = NetworkTopologyGraph::default();
        graph = facts.mutate(graph, &mut g).unwrap();
        assert_eq!(graph.strict_partitions(), 1);
    }

    /// Test that we can build a network with a dozen nodes and three partitions.
    #[test]
    fn test_sweet_topos_strictly_partitioned_network_dozen_nodes_three_partitions() {
        let mut g = unstructured_noise().into();
        let size_fact = SizedNetworkFact { nodes: 12 };
        let partition_fact = StrictlyPartitionedNetworkFact {
            partitions: 3,
            efficiency: 0.2,
        };
        // let facts = facts![size_fact, partition_fact];
        let mut graph = NetworkTopologyGraph::default();
        graph = size_fact.mutate(graph, &mut g).unwrap();
        println!(
            "{:?}",
            Dot::with_config(
                graph.as_ref(),
                &[
                    //     Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        );
        graph = partition_fact.mutate(graph, &mut g).unwrap();
        assert_eq!(connected_components(graph.as_ref()), 3);

        println!(
            "{:?}",
            Dot::with_config(
                graph.as_ref(),
                &[
                    //     Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        );
    }
}
