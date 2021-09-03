use std::sync::Arc;

use super::{SweetAgents, SweetAppBatch, SweetConductor};
use crate::{
    conductor::{config::ConductorConfig, handle::DevSettingsDelta},
    test_utils::gossip_fixtures::GOSSIP_FIXTURE_OP_LOOKUP,
};
use hdk::prelude::*;
use holochain_p2p::*;
use holochain_types::prelude::*;
use kitsune_p2p::test_util::scenario_def::{LocBucket, PeerMatrix, ScenarioDef};

use holochain_conductor_api::conductor::TestConfig;

/// Represents a single node in a sharded gossip scenario.
#[derive(Debug, shrinkwraprs::Shrinkwrap)]
pub struct SweetGossipScenarioNode {
    #[shrinkwrap(main_field)]
    conductor: SweetConductor,
    apps: SweetAppBatch,
    excluded_ops: Arc<HashSet<DhtOpHash>>,
}

impl SweetGossipScenarioNode {
    /// Get the LocBuckets for every op hash held by this node.
    /// Ops which were not manually injected are excluded.
    pub async fn get_op_basis_loc_buckets(&self) -> HashSet<LocBucket> {
        let hashes: HashSet<DhtOpHash> = self
            .conductor
            .get_all_op_hashes(self.apps.cells_flattened())
            .await
            .collect();
        // Exclude the ops which were present at the moment of op injection
        hashes
            .difference(&*self.excluded_ops)
            .map(|h| {
                let loc = *GOSSIP_FIXTURE_OP_LOOKUP.get(&h).unwrap_or_else(|| {
                    panic!(
                        "Must only fixture op hashes for LocBucket lookup. Hash: {}",
                        h
                    )
                });
                loc
            })
            .collect()
    }
}

/// Represents a multi-node sharded gossip scenario, as specified by `ScenarioDef`.
#[derive(Debug)]
pub struct SweetGossipScenario<const N: usize> {
    nodes: [SweetGossipScenarioNode; N],
    excluded_ops: Arc<HashSet<DhtOpHash>>,
}

impl<const N: usize> SweetGossipScenario<N> {
    /// Create a ConductorBatch from a kitsune `ScenarioDef`.
    /// The resulting conductors will have the specified DNAs installed as an app,
    /// and be pre-seeded with agents and op data as specified by the scenario.
    /// The provided DnaFile must
    pub async fn setup(scenario: ScenarioDef<N>, dna_file: DnaFile) -> Self {
        let mut conductors_with_apps = Vec::with_capacity(N);
        let mut excluded_ops = HashSet::new();
        let node_iter = itertools::zip(scenario.nodes.iter(), std::iter::repeat(dna_file.clone()));
        for (i, (node, dna_file)) in node_iter.enumerate() {
            let mut conductor = SweetConductor::from_config(sharded_config()).await;
            conductor.update_dev_settings(DevSettingsDelta {
                publish: Some(false),
                ..Default::default()
            });
            let agent_defs: Vec<_> = node.agents.iter().collect();
            let agents = SweetAgents::get(conductor.keystore(), agent_defs.len()).await;
            let apps = conductor
                .setup_app_for_agents(
                    &format!("node-{}", i),
                    agents.as_slice(),
                    &[dna_file.clone()],
                )
                .await
                .expect("Scenario setup is infallible");

            // TODO: remove!
            conductor.solidify_environment();

            // Record existing op hashes which were created during genesis,
            // so that these can later be filtered out.
            let cells = apps.cells_flattened();
            excluded_ops.extend(conductor.get_all_op_hashes(cells.clone()).await);

            for (agent_def, cell) in itertools::zip(agent_defs, cells) {
                // Manually set the storage arc
                cell.set_storage_arc(agent_def.arc()).await;
                // Manually inject DhtOps at the correct locations
                cell.inject_gossip_fixture_ops(agent_def.ops.clone().into_iter());
            }

            conductors_with_apps.push((conductor, apps));
        }

        let excluded_ops = Arc::new(excluded_ops);
        let nodes: Vec<_> = conductors_with_apps
            .into_iter()
            .map(|(conductor, apps)| SweetGossipScenarioNode {
                conductor,
                apps,
                excluded_ops: excluded_ops.clone(),
            })
            .collect();

        let conductors: Vec<&SweetConductor> = nodes.iter().map(|n| &n.conductor).collect();

        // Inject agent infos according to the PeerMatrix
        match scenario.peer_matrix {
            PeerMatrix::Full => SweetConductor::exchange_peer_info(conductors.clone()).await,
            PeerMatrix::Sparse(matrix) => {
                for (i, conductor) in conductors.iter().enumerate() {
                    conductor
                        .inject_peer_info(
                            matrix[i].iter().map(|c| conductors[*c]),
                            dna_file.dna_hash().to_owned(),
                        )
                        .await;
                }
            }
        };

        Self {
            nodes: nodes.try_into().expect("Total nodes must match input"),
            excluded_ops,
        }
    }

    /// Get references to the nodes. Can be destructured with array syntax.
    pub fn nodes(&self) -> [&SweetGossipScenarioNode; N] {
        self.nodes.iter().collect::<Vec<_>>().try_into().unwrap()
    }
}

fn sharded_config() -> ConductorConfig {
    use holochain_conductor_api::*;
    use kitsune_p2p::*;
    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();
    let mut network = KitsuneP2pConfig::default();
    network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    network.tuning_params = std::sync::Arc::new(tuning);
    ConductorConfig {
        network: Some(network),
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port: 0 },
        }]),
        test: TestConfig {},
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use kitsune_p2p::test_util::scenario_def::*;
    use maplit::hashset;

    use crate::test_utils::inline_zomes::unit_dna;

    use super::*;

    use kitsune_p2p::test_util::scenario_def::ScenarioDefAgent as Agent;
    use kitsune_p2p::test_util::scenario_def::ScenarioDefNode as Node;

    /// Just test that a scenario can be instantiated and results in the proper
    /// conductor state being created
    #[tokio::test(flavor = "multi_thread")]
    async fn scenario_smoke_test_single_node() {
        let def = ScenarioDef::new(
            [Node::new([
                Agent::new((0, 110), [1, 2, 3]),
                Agent::new((-30, 90), [4, 5, 6]),
            ])],
            PeerMatrix::Full,
        );
        let dna_file = unit_dna().await;
        let scenario = SweetGossipScenario::setup(def, dna_file).await;
        let [node] = scenario.nodes();

        let ops0 = node.get_op_basis_loc_buckets().await;

        // - Check that the specially prepared ops are present
        assert_eq!(ops0, hashset![1, 2, 3, 4, 5, 6]);
    }

    /// Just test that a scenario can be instantiated and results in the proper
    /// conductor state being created
    #[tokio::test(flavor = "multi_thread")]
    async fn scenario_smoke_test_two_nodes() {
        let def = ScenarioDef::new(
            [
                Node::new([
                    Agent::new((0, 110), [0, 10, 20, 30, 90]),
                    Agent::new((-30, 90), [90, 80, -10]),
                ]),
                Node::new([
                    Agent::new((0, 110), [5, 15, 25, 35, 95]),
                    Agent::new((-30, 90), [75, 85, -25]),
                ]),
            ],
            PeerMatrix::sparse([&[1], &[]]),
        );
        let dna_file = unit_dna().await;
        let scenario = SweetGossipScenario::setup(def, dna_file).await;
        let [node0, node1] = scenario.nodes();

        // - All local and remote agents are available on the first conductor
        assert_eq!(node0.get_agent_infos(None).await.unwrap().len(), 4);
        // - Only local agents are available on the second conductor
        assert_eq!(node1.get_agent_infos(None).await.unwrap().len(), 2);

        let ops0 = node0.get_op_basis_loc_buckets().await;
        let ops1 = node1.get_op_basis_loc_buckets().await;

        // - Check that the specially prepared ops are present
        assert_eq!(ops0, hashset![0, 10, 20, 30, 90, 80, -10]);
        assert_eq!(ops1, hashset![5, 15, 25, 35, 75, 85, 95, -25]);
    }
}
