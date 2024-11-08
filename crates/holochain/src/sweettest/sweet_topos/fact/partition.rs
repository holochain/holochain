use crate::sweettest::sweet_topos::network::NetworkTopology;
use contrafact::Fact;
use contrafact::Generator;
use contrafact::Mutation;
use rand::Rng;

/// Fact:
/// - The network has a specific number of partitions.
/// - The network is partitioned as strictly as possible. This means that there
///   are no edges between nodes in different partitions.
/// - The partition generation process has a specific efficiency. More efficient
///   partitioning means that the partitions heal more quickly which can lead to
///   one or a few partitions dominating the network. Less efficient partitioning
///   means that the partitions heal more slowly which can lead to a more even
///   distribution of partitions.
#[derive(Clone, Debug)]
pub struct StrictlyPartitionedNetworkFact {
    /// The number of partitions in the network.
    pub partitions: usize,
    /// The efficiency of the partitioning process. This is a number between 0
    /// and 1. The higher the number, the more efficient the partitioning
    /// process.
    pub efficiency: f64,
}

impl<'a> Fact<'a, NetworkTopology> for StrictlyPartitionedNetworkFact {
    fn mutate(
        &mut self,
        g: &mut Generator<'a>,
        mut graph: NetworkTopology,
    ) -> Mutation<NetworkTopology> {
        let mut rng = super::rng_from_generator(g);
        let efficiency_cutoff = (self.efficiency * u64::MAX as f64) as u64;

        // Remove edges until the graph is partitioned into the desired number of
        // partitions. The edges are removed randomly, so this is not the most
        // efficient way to do this, but it's simple and it works.
        while graph.strict_partitions() < self.partitions {
            graph.remove_random_edge(&mut rng)?;
        }

        // Add edges until the graph is connected up to the desired number of
        // partitions.
        while graph.strict_partitions() > self.partitions {
            // If the efficiency switch is above the cutoff, we'll reassign an
            // existing node to a different partition. Otherwise, we'll add a new
            // edge between two nodes in different partitions.
            // We can't reassign a node to a different partition if there's only
            // one desired partition, so we'll just add an edge in that case.
            let efficiency_switch = rng.gen::<u64>();
            if efficiency_switch > efficiency_cutoff && self.partitions > 1 {
                graph.reassign_random_node_to_smallest_strict_partition(&mut rng)?;
            } else {
                graph.heal_random_strict_partition(&mut rng)?;
            }
        }

        Ok(graph)
    }

    fn label(&self) -> String {
        todo!()
    }

    fn labeled(self, _: impl ToString) -> Self {
        todo!()
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::prelude::unstructured_noise;
    use crate::sweettest::fact::partition::StrictlyPartitionedNetworkFact;
    use crate::sweettest::fact::size::SizedNetworkFact;
    use contrafact::facts;
    use contrafact::Fact;
    use petgraph::algo::connected_components;
    use petgraph::dot::{Config, Dot};

    /// Test that we can build a network with one partition.
    #[test]
    fn test_sweet_topos_strictly_partitioned_network_one_partition() {
        crate::big_stack_test!(async move {
            let mut g = unstructured_noise().into();
            let size_fact = SizedNetworkFact {
                nodes: 3,
                agents: 3..=5,
            };
            let partition_fact = StrictlyPartitionedNetworkFact {
                partitions: 1,
                efficiency: 1.0,
            };
            let mut facts = facts![size_fact, partition_fact];
            let mut graph = NetworkTopology::default();
            graph = facts.mutate(&mut g, graph).unwrap();
            assert_eq!(graph.strict_partitions(), 1);
        });
    }

    /// Test that we can build a network with a dozen nodes and three partitions.
    #[test]
    fn test_sweet_topos_strictly_partitioned_network_dozen_nodes_three_partitions() {
        crate::big_stack_test!(async move {
            let mut g = unstructured_noise().into();
            let mut size_fact = SizedNetworkFact {
                nodes: 12,
                agents: 1..=2,
            };
            let mut partition_fact = StrictlyPartitionedNetworkFact {
                partitions: 3,
                efficiency: 0.2,
            };
            // let facts = facts![size_fact, partition_fact];
            let mut graph = NetworkTopology::default();
            graph = size_fact.mutate(&mut g, graph).unwrap();
            tracing::info!(
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
            graph = partition_fact.mutate(&mut g, graph).unwrap();
            assert_eq!(connected_components(graph.as_ref()), 3);

            tracing::info!(
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
        });
    }
}
