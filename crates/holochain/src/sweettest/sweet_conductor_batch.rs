use super::{standard_config, SweetAgents, SweetAppBatch, SweetConductor};
use crate::conductor::{api::error::ConductorApiResult, config::ConductorConfig};
use ::fixt::prelude::StdRng;
use futures::future;
use hdk::prelude::*;
use holochain_types::prelude::*;
use rand::Rng;
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

    /// Get the conductor at an index.
    pub fn get(&self, i: usize) -> Option<&SweetConductor> {
        self.0.get(i)
    }

    /// Add an existing conductor to this batch
    pub fn add_conductor(&mut self, c: SweetConductor) {
        self.0.push(c);
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

    /// Let each conductor know about each others' agents so they can do networking
    pub async fn exchange_peer_info(&self) {
        let mut all = Vec::new();
        for c in self.0.iter() {
            for env in c.spaces.get_from_spaces(|s| s.p2p_agents_db.clone()) {
                all.push(env.clone());
            }
        }
        crate::conductor::p2p_agent_store::exchange_peer_info(all).await;
    }

    /// Let each conductor know about each others' agents so they can do networking
    pub async fn exchange_peer_info_sampled(&self, rng: &mut StdRng, s: usize) {
        let mut all = Vec::new();
        for c in self.0.iter() {
            for env in c.spaces.get_from_spaces(|s| s.p2p_agents_db.clone()) {
                all.push(env.clone());
            }
        }
        let connectivity = covering(rng, all.len(), s);
        crate::conductor::p2p_agent_store::exchange_peer_info_sparse(all, connectivity).await;
    }

    /// Let a conductor know about all agents on some other conductor.
    pub async fn reveal_peer_info(&self, observer: usize, seen: usize) {
        let observer_conductor = &self.0[observer];
        let mut observer_envs = Vec::new();
        for env in observer_conductor
            .spaces
            .get_from_spaces(|s| s.p2p_agents_db.clone())
        {
            observer_envs.push(env.clone());
        }

        let seen_conductor = &self.0[seen];
        let mut seen_envs = Vec::new();
        for env in seen_conductor
            .spaces
            .get_from_spaces(|s| s.p2p_agents_db.clone())
        {
            seen_envs.push(env.clone());
        }

        crate::conductor::p2p_agent_store::reveal_peer_info(observer_envs, seen_envs).await;
    }

    /// Force trigger all dht ops that haven't received
    /// enough validation receipts yet.
    pub async fn force_all_publish_dht_ops(&self) {
        for c in self.0.iter() {
            c.force_all_publish_dht_ops().await;
        }
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

fn covering(rng: &mut StdRng, n: usize, s: usize) -> Vec<HashSet<usize>> {
    let nodes: Vec<_> = (0..n)
        .map(|i| {
            let peers: HashSet<_> = std::iter::repeat_with(|| rng.gen_range(0..n))
                .filter(|j| i != *j)
                .take(s)
                .collect();
            peers
        })
        .collect();
    let mut visited = HashSet::<usize>::new();
    let mut queue = vec![0];
    while let Some(next) = queue.pop() {
        let unvisited: Vec<_> = nodes[next]
            .iter()
            .filter(|p| !visited.contains(p))
            .copied()
            .collect();
        queue.extend(unvisited.iter());
        visited.extend(unvisited.iter());
        if visited.len() == n {
            break;
        }
    }
    if visited.len() < n {
        panic!("Covering could not be created. Try a higher s value.");
    }
    nodes
}
