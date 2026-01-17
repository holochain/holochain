use super::{SweetAppBatch, SweetConductor, SweetConductorConfig};
use crate::conductor::api::error::ConductorApiResult;
use crate::sweettest::*;
use futures::future;
use hdk::prelude::*;
use holochain_types::prelude::*;
use kitsune2_api::LocalAgent;
use std::path::PathBuf;

/// A collection of SweetConductors, with methods for operating on the entire collection
#[derive(derive_more::Into, derive_more::IntoIterator, derive_more::Deref)]
pub struct SweetConductorBatch(Vec<SweetConductor>);

impl SweetConductorBatch {
    /// Constructor with validation
    pub fn new(conductors: Vec<SweetConductor>) -> Self {
        let paths: HashSet<PathBuf> = conductors
            .iter()
            .filter_map(|c| {
                c.config
                    .data_root_path
                    .as_ref()
                    .map(|data_path| data_path.as_ref().clone())
            })
            .collect();
        assert_eq!(
            conductors.len(),
            paths.len(),
            "Some conductors in a SweetConductorBatch share the same data path (or don't have a path)!"
        );
        Self(conductors)
    }

    /// Map the given ConductorConfigs into SweetConductors, each with its own new TestEnvironments
    #[allow(clippy::let_and_return)]
    pub async fn from_configs<C, I>(configs: I) -> SweetConductorBatch
    where
        C: Into<SweetConductorConfig>,
        I: IntoIterator<Item = C>,
    {
        Self::new(
            future::join_all(configs.into_iter().map(|c| SweetConductor::from_config(c))).await,
        )
    }

    /// Create SweetConductors from the given ConductorConfigs, each with its own new TestEnvironments,
    /// using a "rendezvous" bootstrap server for peer discovery.
    #[allow(clippy::let_and_return)]
    pub async fn from_configs_rendezvous<C, I>(configs: I) -> SweetConductorBatch
    where
        C: Into<SweetConductorConfig>,
        I: IntoIterator<Item = C>,
    {
        let rendezvous = SweetLocalRendezvous::new().await;
        Self::new(
            future::join_all(
                configs
                    .into_iter()
                    .map(|c| SweetConductor::from_config_rendezvous(c, rendezvous.clone())),
            )
            .await,
        )
    }

    /// Create the given number of new SweetConductors, each with its own new TestEnvironments
    pub async fn from_config<C: Clone + Into<SweetConductorConfig>>(
        num: usize,
        config: C,
    ) -> SweetConductorBatch {
        let config = config.into();
        Self::from_configs(std::iter::repeat_n(config, num)).await
    }

    /// Create a number of SweetConductors from the given ConductorConfig, each with its own new TestEnvironments.
    /// using a "rendezvous" bootstrap server for peer discovery.
    pub async fn from_config_rendezvous<C>(num: usize, config: C) -> SweetConductorBatch
    where
        C: Into<SweetConductorConfig> + Clone,
    {
        Self::from_configs_rendezvous(std::iter::repeat_n(config, num)).await
    }

    /// Create the given number of new SweetConductors, each with its own new TestEnvironments
    pub async fn standard(num: usize) -> SweetConductorBatch {
        Self::from_config_rendezvous(num, SweetConductorConfig::rendezvous(true)).await
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

    /// Create and add a new conductor to this batch
    pub async fn add_conductor_from_config<C>(&mut self, c: C)
    where
        C: Into<SweetConductorConfig>,
    {
        let conductor =
            if let Some(rendezvous) = self.0.first().and_then(|c| c.get_rendezvous_config()) {
                SweetConductor::from_config_rendezvous(c, rendezvous).await
            } else {
                SweetConductor::from_config(c).await
            };

        self.0.push(conductor);
    }

    /// Opinionated app setup.
    /// Creates one app on each Conductor in this batch, creating a new AgentPubKey for each.
    /// The created AgentPubKeys can be retrieved via each SweetApp.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn setup_app<'a>(
        &mut self,
        installed_app_id: &str,
        dnas_with_roles: impl IntoIterator<Item = &'a (impl DnaWithRole + 'a)> + Clone,
    ) -> ConductorApiResult<SweetAppBatch> {
        let apps = self
            .0
            .iter_mut()
            .map(|conductor| {
                let dnas_with_roles = dnas_with_roles.clone();
                async move { conductor.setup_app(installed_app_id, dnas_with_roles).await }
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
    pub async fn setup_app_for_zipped_agents<'a>(
        &mut self,
        installed_app_id: &str,
        agents: impl IntoIterator<Item = &AgentPubKey> + Clone,
        dna_files: impl IntoIterator<Item = &'a (impl DnaWithRole + 'a)> + Clone,
    ) -> ConductorApiResult<SweetAppBatch> {
        if agents.clone().into_iter().count() != self.0.len() {
            panic!(
                "setup_app_for_zipped_agents must take as many Agents as there are Conductors in this batch."
            )
        }

        let apps = self
            .0
            .iter_mut()
            .zip(agents.into_iter())
            .map(|(conductor, agent)| {
                conductor.setup_app_for_agent(installed_app_id, agent.clone(), dna_files.clone())
            })
            .collect::<Vec<_>>();

        Ok(future::join_all(apps)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .into())
    }

    /// Let each conductor know about each other's agents so they can do networking
    pub async fn exchange_peer_info(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(10), async move {
            while !SweetConductor::exchange_peer_info(&self.0).await {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("Timeout while exchanging peer info");
    }

    /// Let a conductor know about all agents on some other conductor.
    ///
    /// Copies from seen to observer.
    pub async fn reveal_peer_info(&self, observer: usize, seen: usize) {
        let observer_conductor = &self.0[observer];
        let mut observer_dna_hashes = Vec::new();
        for env in observer_conductor
            .spaces
            .get_from_spaces(|s| s.dna_hash.clone())
        {
            observer_dna_hashes.push(env.clone());
        }

        let seen_conductor = &self.0[seen];
        let mut seen_dna_hashes = Vec::new();
        for env in seen_conductor
            .spaces
            .get_from_spaces(|s| s.dna_hash.clone())
        {
            seen_dna_hashes.push(env.clone());
        }

        for dna_hash in seen_dna_hashes {
            let from_local_agents = seen_conductor
                .holochain_p2p()
                .test_kitsune()
                .space_if_exists(dna_hash.to_k2_space())
                .await
                .unwrap()
                .local_agent_store()
                .get_all()
                .await
                .unwrap();

            let from_peer_store = seen_conductor
                .holochain_p2p()
                .peer_store((*dna_hash).clone())
                .await
                .unwrap();

            let mut agent_infos_for_local_agents = Vec::with_capacity(from_local_agents.len());
            for local_agent in from_local_agents {
                let agent_info = from_peer_store
                    .get(local_agent.agent().clone())
                    .await
                    .unwrap();

                if let Some(agent_info) = agent_info {
                    agent_infos_for_local_agents.push(agent_info);
                }
            }

            observer_conductor
                .holochain_p2p()
                .peer_store((*dna_hash).clone())
                .await
                .unwrap()
                .insert(agent_infos_for_local_agents)
                .await
                .unwrap();
        }
    }

    /// Make the temp db dir persistent
    pub fn persist_dbs(&mut self) {
        for c in self.0.iter_mut() {
            let _ = c.persist_dbs();
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
