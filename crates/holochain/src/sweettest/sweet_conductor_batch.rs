use super::{standard_config, SweetAgents, SweetAppBatch, SweetConductor};
use crate::conductor::{
    api::error::ConductorApiResult, config::ConductorConfig, handle::DevSettingsDelta,
};
use futures::future;
use hdk::prelude::*;
use holochain_types::prelude::*;

#[cfg(any(test, feature = "test_utils"))]
use holochain_conductor_api::conductor::TestConfig;

/// A collection of SweetConductors, with methods for operating on the entire collection
#[derive(derive_more::From, derive_more::Into, derive_more::IntoIterator)]
pub struct SweetConductorBatch(Vec<SweetConductor>);

impl SweetConductorBatch {
    /// Map the given ConductorConfigs into SweetConductors, each with its own new TestEnvironments
    pub async fn from_configs<I: IntoIterator<Item = ConductorConfig>>(
        configs: I,
    ) -> SweetConductorBatch {
        future::join_all(configs.into_iter().map(SweetConductor::from_config))
            .await
            .into()
    }

    /// Create the given number of new SweetConductors, each with its own new TestEnvironments
    pub async fn from_config(num: usize, config: ConductorConfig) -> SweetConductorBatch {
        Self::from_configs(std::iter::repeat(config).take(num)).await
    }

    /// Create the given number of new SweetConductors, each with its own new TestEnvironments
    pub async fn from_standard_config(num: usize) -> SweetConductorBatch {
        Self::from_configs(std::iter::repeat_with(standard_config).take(num)).await
    }

    /// Iterate over the SweetConductors
    pub fn iter(&self) -> impl Iterator<Item = &SweetConductor> {
        self.0.iter()
    }

    /// Iterate over the SweetConductors, mutably
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SweetConductor> {
        self.0.iter_mut()
    }

    /// Convert to a Vec
    pub fn into_inner(self) -> Vec<SweetConductor> {
        self.0
    }

    /// Opinionated app setup.
    /// Creates one app on each Conductor in this batch, creating a new AgentPubKey for each.
    /// The created AgentPubKeys can be retrieved via each SweetApp.
    pub async fn setup_app(
        &mut self,
        installed_app_id: &str,
        dna_files: &[DnaFile],
    ) -> ConductorApiResult<SweetAppBatch> {
        let apps = self
            .0
            .iter_mut()
            .map(|conductor| async move {
                let agent = SweetAgents::one(conductor.keystore()).await;
                conductor
                    .setup_app_for_agent(installed_app_id, agent, dna_files)
                    .await
            })
            .collect::<Vec<_>>();

        Ok(future::join_all(apps)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .into())
    }

    /// Opinionated app setup. Creates one app on each Conductor in this batch,
    /// using the given agents and DnaFiles.
    ///
    /// The number of Agents passed in must be the same as the number of Conductors
    /// in this batch. Each Agent will be used to create one app on one Conductor,
    /// hence the "zipped" in the function name
    ///
    /// Returns a batch of SweetApps, sorted in the same order as the Conductors in
    /// this batch.
    pub async fn setup_app_for_zipped_agents(
        &mut self,
        installed_app_id: &str,
        agents: &[AgentPubKey],
        dna_files: &[DnaFile],
    ) -> ConductorApiResult<SweetAppBatch> {
        if agents.len() != self.0.len() {
            panic!(
                "setup_app_for_zipped_agents must take as many Agents as there are Conductors in this batch."
            )
        }

        let apps = self
            .0
            .iter_mut()
            .zip(agents.iter())
            .map(|(conductor, agent)| {
                conductor.setup_app_for_agent(installed_app_id, agent.clone(), dna_files)
            })
            .collect::<Vec<_>>();

        Ok(future::join_all(apps)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .into())
    }
}

use holochain_p2p::*;
use kitsune_p2p::test_util::scenario_def::{PeerMatrix, ScenarioDef};

impl SweetConductorBatch {
    /// Create a ConductorBatch from a kitsune `ScenarioDef`.
    /// The resulting conductors will have the specified DNAs installed as an app,
    /// and be pre-seeded with agents and op data as specified by the scenario.
    /// The provided DnaFile must
    pub async fn setup_from_scenario<const N: usize>(
        scenario: ScenarioDef<N>,
        dna_file: DnaFile,
    ) -> [(SweetConductor, SweetAppBatch); N] {
        let mut conductors_and_apps = Vec::with_capacity(N);
        let mut genesis_op_hashes = HashSet::new();
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
            genesis_op_hashes.extend(conductor.get_all_op_hashes(cells.clone()).await);

            for (agent_def, cell) in itertools::zip(agent_defs, cells) {
                // Manually set the storage arc
                cell.set_storage_arc(agent_def.arc()).await;
                // Manually inject DhtOps at the correct locations
                cell.populate_fixture_ops(agent_def.ops.clone().into_iter());
            }

            conductors_and_apps.push((conductor, apps));
        }

        let conductors: Vec<&SweetConductor> = conductors_and_apps.iter().map(|(c, _)| c).collect();

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

        conductors_and_apps
            .try_into()
            .expect("Total conductors must match input")
    }

    /// Let each conductor know about each others' agents so they can do networking
    pub async fn exchange_peer_info(&self) {
        let mut all = Vec::new();
        for c in self.0.iter() {
            for env in c.envs().p2p().lock().values() {
                all.push(env.clone());
            }
        }
        crate::conductor::p2p_agent_store::exchange_peer_info(all).await;
    }
}

impl std::ops::Index<usize> for SweetConductorBatch {
    type Output = SweetConductor;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for SweetConductorBatch {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
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
    use maplit::hashset;

    use crate::test_utils::inline_zomes::unit_dna;

    use super::*;

    use kitsune_p2p::test_util::scenario_def::ScenarioDefAgent as Agent;
    use kitsune_p2p::test_util::scenario_def::ScenarioDefNode as Node;

    /// Just test that a scenario can be instantiated and results in the proper
    /// conductor state being created
    #[tokio::test(flavor = "multi_thread")]
    async fn scenario_smoke_test_single_node() {
        let scenario = ScenarioDef::new(
            [Node::new([
                Agent::new((0, 110), [1, 2, 3]),
                Agent::new((-30, 90), [4, 5, 6]),
            ])],
            PeerMatrix::Full,
        );
        let dna_file = unit_dna().await;
        let [(conductor0, apps0)] =
            SweetConductorBatch::setup_from_scenario(scenario, dna_file).await;

        let ops0 = conductor0.get_op_basis_buckets(&apps0).await;

        // - Check that the specially prepared ops are present
        assert_eq!(ops0, hashset![1, 2, 3, 4, 5, 6]);
    }

    /// Just test that a scenario can be instantiated and results in the proper
    /// conductor state being created
    #[tokio::test(flavor = "multi_thread")]
    async fn scenario_smoke_test_two_nodes() {
        let scenario = ScenarioDef::new(
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
        let [(conductor0, apps0), (conductor1, apps1)] =
            SweetConductorBatch::setup_from_scenario(scenario, dna_file).await;

        // - All local and remote agents are available on the first conductor
        assert_eq!(conductor0.get_agent_infos(None).await.unwrap().len(), 4);
        // - Only local agents are available on the second conductor
        assert_eq!(conductor1.get_agent_infos(None).await.unwrap().len(), 2);

        let ops0 = conductor0.get_op_basis_buckets(&apps0).await;
        let ops1 = conductor1.get_op_basis_buckets(&apps1).await;
        dbg!(&ops0, &ops1);

        // - Check that the specially prepared ops are present
        assert_eq!(ops0, hashset![0, 10, 20, 30, 90, 80, -10]);
        assert_eq!(ops1, hashset![5, 15, 25, 35, 75, 85, -25]);
    }
}
