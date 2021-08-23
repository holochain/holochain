use super::{standard_config, SweetAgents, SweetAppBatch, SweetConductor};
use crate::conductor::{api::error::ConductorApiResult, config::ConductorConfig};
use futures::future;
use hdk::prelude::*;
use holochain_types::prelude::*;

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

#[cfg(feature = "no-hash-integrity")]
use holochain_p2p::*;
#[cfg(feature = "no-hash-integrity")]
use kitsune_p2p::test_util::scenario_def::{PeerMatrix, ScenarioDef};

#[cfg(feature = "no-hash-integrity")]
impl SweetConductorBatch {
    /// Create a ConductorBatch from a kitsune `ScenarioDef`.
    /// The resulting conductors will have the specified DNAs installed as an app,
    /// and be pre-seeded with agents and op data as specified by the scenario.
    /// The provided DnaFile must
    pub async fn setup_from_scenario<const N: usize>(
        scenario: ScenarioDef<N>,
        dna_file: DnaFile,
    ) -> [(SweetConductor, SweetAppBatch); N] {
        let tasks = itertools::zip(scenario.nodes.iter(), std::iter::repeat(dna_file.clone()))
            .enumerate()
            .map(|(i, (node, dna_file))| async move {
                let mut conductor = SweetConductor::from_standard_config().await;
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

                for (agent_def, cell) in itertools::zip(agent_defs, apps.cells_flattened()) {
                    // Manually set the storage arc
                    cell.set_storage_arc(agent_def.arc.clone()).await;
                    // Manually inject DhtOps at the correct locations
                    cell.inject_fake_ops(agent_def.ops.clone().into_iter());
                }

                (conductor, apps)
            });
        let conductors_and_apps: Vec<_> = future::join_all(tasks)
            .await
            .try_into()
            .unwrap_or_else(|_| unreachable!("Array size must match"));

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

#[cfg(test)]
#[cfg(feature = "no-hash-integrity")]
mod tests {
    use maplit::hashset;

    use crate::test_utils::inline_zomes::unit_dna;

    use super::*;

    /// Just test that a scenario can be instantiated and results in the proper
    /// conductor state being created
    #[tokio::test(flavor = "multi_thread")]
    async fn scenario_smoke_test() {
        use kitsune_p2p::dht_arc::ArcInterval;
        use kitsune_p2p::test_util::scenario_def::ScenarioDefAgent as Agent;
        use kitsune_p2p::test_util::scenario_def::ScenarioDefNode as Node;

        let scenario = ScenarioDef::new(
            [
                Node::new(hashset![
                    Agent::new(ArcInterval::new(0, 110), [0, 10, 20, 30, 90]),
                    Agent::new(ArcInterval::new(90, 200), [90, 100, 150]),
                ]),
                Node::new(hashset![
                    Agent::new(ArcInterval::new(0, 110), [5, 15, 25, 35, 95]),
                    Agent::new(ArcInterval::new(90, 200), [95, 105, 155]),
                ]),
            ],
            PeerMatrix::sparse([&[1], &[]]),
        );
        let dna_file = unit_dna().await;
        let dna_hash = dna_file.dna_hash().clone();
        let [(conductor0, apps0), (conductor1, apps1)] =
            SweetConductorBatch::setup_from_scenario(scenario, dna_file).await;

        // - All local and remote agents are available on the first conductor
        assert_eq!(conductor0.get_agent_infos(None).await.unwrap().len(), 4);
        // - Only local agents are available on the second conductor
        assert_eq!(conductor1.get_agent_infos(None).await.unwrap().len(), 2);

        let to_agents_0: Vec<_> = apps0
            .cells_flattened()
            .into_iter()
            .map(|cell| cell.agent_pubkey().clone())
            .collect();
        let to_agents_1: Vec<_> = apps1
            .cells_flattened()
            .into_iter()
            .map(|cell| cell.agent_pubkey().clone())
            .collect();
        let ops0: HashSet<_> = conductor0
            .get_all_op_hashes(&dna_hash, to_agents_0)
            .await
            .into_iter()
            .map(|h| h.get_loc().as_u32())
            .collect();
        let ops1: HashSet<_> = conductor1
            .get_all_op_hashes(&dna_hash, to_agents_1)
            .await
            .into_iter()
            .map(|h| h.get_loc().as_u32())
            .collect();

        // - Check that the specially prepared ops are present
        //   (must check for subset because the usual genesis ops are still created)
        assert!(ops0.is_superset(&hashset![0, 10, 20, 30, 90, 100, 150]));
        assert!(ops1.is_superset(&hashset![5, 15, 25, 35, 95, 105, 155]));
    }
}
