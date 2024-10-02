use super::{SweetAppBatch, SweetConductor, SweetConductorConfig};
use crate::conductor::api::error::ConductorApiResult;
use crate::sweettest::*;
use ::fixt::prelude::StdRng;
use futures::future;
use hdk::prelude::*;
use holochain_types::prelude::*;
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
    pub async fn from_configs<C, I>(configs: I) -> SweetConductorBatch
    where
        C: Into<SweetConductorConfig>,
        I: IntoIterator<Item = C>,
    {
        let conductors = Self::new(
            future::join_all(configs.into_iter().map(|c| SweetConductor::from_config(c))).await,
        );

        let dpki_cells = conductors.dpki_cells();
        if !dpki_cells.is_empty() {
            conductors.exchange_peer_info().await;
            await_consistency(10, dpki_cells.as_slice()).await.unwrap();
        }

        conductors
    }

    /// Create SweetConductors from the given ConductorConfigs, each with its own new TestEnvironments,
    /// using a "rendezvous" bootstrap server for peer discovery.
    ///
    /// Also await consistency for DPKI cells, if DPKI is enabled.
    pub async fn from_configs_rendezvous<C, I>(configs: I) -> SweetConductorBatch
    where
        C: Into<SweetConductorConfig>,
        I: IntoIterator<Item = C>,
    {
        let rendezvous = SweetLocalRendezvous::new().await;
        let conductors = Self::new(
            future::join_all(
                configs
                    .into_iter()
                    .map(|c| SweetConductor::from_config_rendezvous(c, rendezvous.clone())),
            )
            .await,
        );

        let not_full_bootstrap = conductors
            .iter()
            .any(|c| !c.get_config().has_rendezvous_bootstrap());

        let dpki_cells = conductors.dpki_cells();
        if !dpki_cells.is_empty() {
            // Typically we expect either all nodes are using a rendezvous bootstrap, or none are.
            // To cover all cases, we say if any are not using bootstrap, we'll exchange peer info
            // for everyone, even though this may be incorrect.
            if not_full_bootstrap {
                conductors.exchange_peer_info().await;
            }
            await_consistency(15, dpki_cells.as_slice()).await.unwrap();
        }

        conductors
    }

    /// Create the given number of new SweetConductors, each with its own new TestEnvironments
    pub async fn from_config<C: Clone + Into<SweetConductorConfig>>(
        num: usize,
        config: C,
    ) -> SweetConductorBatch {
        let config = config.into();
        Self::from_configs(std::iter::repeat(config).take(num)).await
    }

    /// Create a number of SweetConductors from the given ConductorConfig, each with its own new TestEnvironments.
    /// using a "rendezvous" bootstrap server for peer discovery.
    ///
    /// Also await consistency for DPKI cells, if DPKI is enabled.
    pub async fn from_config_rendezvous<C>(num: usize, config: C) -> SweetConductorBatch
    where
        C: Into<SweetConductorConfig> + Clone,
    {
        Self::from_configs_rendezvous(std::iter::repeat(config).take(num)).await
    }

    /// Create the given number of new SweetConductors, each with its own new TestEnvironments
    pub async fn from_standard_config(num: usize) -> SweetConductorBatch {
        Self::from_configs(
            std::iter::repeat_with(|| SweetConductorConfig::rendezvous(false)).take(num),
        )
        .await
    }

    /// Create the given number of new SweetConductors, each with its own new TestEnvironments
    pub async fn from_standard_config_rendezvous(num: usize) -> SweetConductorBatch {
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

    /// Let each conductor know about each others' agents so they can do networking
    pub async fn exchange_peer_info(&self) {
        SweetConductor::exchange_peer_info(&self.0).await
    }

    /// Let each conductor know about each others' agents so they can do networking
    pub async fn forget_peer_info(&self, agents_to_forget: impl IntoIterator<Item = &AgentPubKey>) {
        SweetConductor::forget_peer_info(&self.0, agents_to_forget).await
    }

    /// Let each conductor know about each others' agents so they can do networking
    pub async fn exchange_peer_info_sampled(&self, rng: &mut StdRng, s: usize) {
        SweetConductor::exchange_peer_info_sampled(&self.0, rng, s).await
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

    /// Get the DPKI cell for each conductor, if applicable
    pub fn dpki_cells(&self) -> Vec<SweetCell> {
        self.0.iter().filter_map(|c| c.dpki_cell()).collect()
    }

    /// Force trigger all dht ops that haven't received
    /// enough validation receipts yet.
    pub async fn force_all_publish_dht_ops(&self) {
        for c in self.0.iter() {
            c.force_all_publish_dht_ops().await;
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
