//! A wrapper around ConductorHandle with more convenient methods for testing
// TODO [ B-03669 ] move to own crate

use super::*;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::ConductorHandle;
use crate::conductor::{
    api::error::ConductorApiResult, config::ConductorConfig, error::ConductorResult, Conductor,
    ConductorBuilder,
};
use crate::retry_until_timeout;
use ::fixt::prelude::StdRng;
use hdk::prelude::*;
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppAuthenticationRequest, CellInfo, ProvisionedCell,
};
use holochain_keystore::MetaLairClient;
use holochain_state::prelude::test_db_dir;
use holochain_state::source_chain::SourceChain;
use holochain_state::test_utils::TestDir;
use holochain_types::prelude::*;
use holochain_types::websocket::AllowedOrigins;
use holochain_websocket::*;
use kitsune2_api::DhtArc;
use nanoid::nanoid;
use rand::Rng;
use rusqlite::named_params;
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A stream of signals.
pub type SignalStream = Box<dyn tokio_stream::Stream<Item = Signal> + Send + Sync + Unpin>;

/// A useful Conductor abstraction for testing, allowing startup and shutdown as well
/// as easy installation of apps across multiple Conductors and Agents.
///
/// This is intentionally NOT `Clone`, because the drop handle triggers a shutdown of
/// the conductor handle, which would render all other cloned instances useless,
/// as well as the fact that the SweetConductor has some extra state which would not
/// be tracked by cloned instances.
/// If you need multiple references to a SweetConductor, put it in an Arc
#[derive(derive_more::From)]
pub struct SweetConductor {
    handle: Option<SweetConductorHandle>,
    db_dir: TestDir,
    keystore: MetaLairClient,
    config: Arc<ConductorConfig>,
    dnas: Vec<DnaFile>,
    rendezvous: Option<DynSweetRendezvous>,
}

/// ID based equality is good for SweetConductors so we can track them
/// independently no matter what kind of mutations/state might eventuate.
impl PartialEq for SweetConductor {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for SweetConductor {}

impl SweetConductor {
    /// Get the ID of this conductor for manual equality checks.
    pub fn id(&self) -> String {
        self.config
            .tracing_scope()
            .expect("SweetConductor must have a tracing scope set")
    }

    /// Update the config if the conductor is shut down
    pub fn update_config(&mut self, f: impl FnOnce(ConductorConfig) -> ConductorConfig) {
        if self.is_running() {
            panic!("Cannot update config while conductor is running");
        }
        self.config = Arc::from(f((*self.config).clone()));
    }

    /// Create a SweetConductor from an already-built ConductorHandle and environments
    /// RibosomeStore
    /// The conductor will be supplied with a single test AppInterface named
    /// "sweet-interface" so that signals may be emitted
    pub async fn new(
        handle: ConductorHandle,
        env_dir: TestDir,
        config: Arc<ConductorConfig>,
        rendezvous: Option<DynSweetRendezvous>,
    ) -> SweetConductor {
        let keystore = handle.keystore().clone();

        Self {
            handle: Some(SweetConductorHandle(handle)),
            db_dir: env_dir,
            keystore,
            config,
            dnas: Vec::new(),
            rendezvous,
        }
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn from_config<C>(config: C) -> SweetConductor
    where
        C: Into<SweetConductorConfig>,
    {
        let config: SweetConductorConfig = config.into();
        let vous = config.get_rendezvous();
        Self::create_with_defaults(config, None, vous).await
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn from_config_rendezvous<C, R>(config: C, rendezvous: R) -> SweetConductor
    where
        C: Into<SweetConductorConfig>,
        R: Into<DynSweetRendezvous> + Clone,
    {
        Self::create_with_defaults(config, None, Some(rendezvous)).await
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn create_with_defaults<C, R>(
        config: C,
        keystore: Option<MetaLairClient>,
        rendezvous: Option<R>,
    ) -> SweetConductor
    where
        C: Into<SweetConductorConfig>,
        R: Into<DynSweetRendezvous> + Clone,
    {
        Self::create_with_defaults_and_metrics(config, keystore, rendezvous, false, false).await
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    /// and a metrics initialization.
    pub async fn create_with_defaults_and_metrics<C, R>(
        config: C,
        keystore: Option<MetaLairClient>,
        rendezvous: Option<R>,
        with_metrics: bool,
        test_builder_uses_production_k2_builder: bool,
    ) -> SweetConductor
    where
        C: Into<SweetConductorConfig>,
        R: Into<DynSweetRendezvous> + Clone,
    {
        let rendezvous = rendezvous.map(|r| r.into());
        let dir = TestDir::new(test_db_dir());

        assert!(
            dir.read_dir().unwrap().next().is_none(),
            "Test dir not empty - {:?}",
            dir.to_path_buf()
        );

        if with_metrics {
            #[cfg(feature = "metrics_influxive")]
            holochain_metrics::HolochainMetricsConfig::new(dir.as_ref())
                .init()
                .await;
        }

        let config: SweetConductorConfig = config.into();
        let mut config: ConductorConfig = if let Some(r) = rendezvous.clone() {
            config
                .tune_network_config(|nc| nc.mem_bootstrap = false)
                .apply_rendezvous(&r)
                .into()
        } else {
            if config
                .network
                .bootstrap_url
                .as_str()
                .starts_with("rendezvous:")
            {
                panic!("Must use rendezvous SweetConductor if rendezvous: is specified in config.network.bootstrap_service");
            }
            if config
                .network
                .signal_url
                .as_str()
                .starts_with("rendezvous:")
            {
                panic!("Must use rendezvous SweetConductor if rendezvous: is specified in config.network.transport_pool[].signal_url");
            }
            config.into()
        };

        if config.tracing_scope().is_none() {
            config.tracing_scope = Some(format!(
                "{}.{}",
                NUM_CREATED.load(Ordering::SeqCst),
                nanoid!(5)
            ));
        }

        if config.data_root_path.is_none() {
            config.data_root_path = Some(dir.as_ref().to_path_buf().into());
        }

        let keystore = keystore.unwrap_or_else(holochain_keystore::test_keystore);

        let handle = Self::handle_from_existing(
            keystore,
            &config,
            &[],
            test_builder_uses_production_k2_builder,
        )
        .await;

        tracing::info!("Starting with config: {:?}", config);

        Self::new(handle, dir, Arc::new(config), rendezvous).await
    }

    /// Create a SweetConductor from a partially-configured ConductorBuilder
    pub async fn from_builder(builder: ConductorBuilder) -> SweetConductor {
        let db_dir = TestDir::new(test_db_dir());
        let builder = builder.with_data_root_path(db_dir.as_ref().to_path_buf().into());
        let config = builder.config.clone();
        let handle = builder.test(&[]).await.unwrap();
        Self::new(handle, db_dir, Arc::new(config), None).await
    }

    /// Create a SweetConductor from a partially-configured ConductorBuilder
    pub async fn from_builder_rendezvous<R>(
        builder: ConductorBuilder,
        rendezvous: R,
    ) -> SweetConductor
    where
        R: Into<DynSweetRendezvous> + Clone,
    {
        let db_dir = TestDir::new(test_db_dir());
        let builder = builder.with_data_root_path(db_dir.as_ref().to_path_buf().into());
        let config = builder.config.clone();
        let handle = builder.test(&[]).await.unwrap();
        Self::new(handle, db_dir, Arc::new(config), Some(rendezvous.into())).await
    }

    /// Create a handle from an existing environment and config
    pub async fn handle_from_existing(
        keystore: MetaLairClient,
        config: &ConductorConfig,
        extra_dnas: &[DnaFile],
        test_builder_uses_production_k2_builder: bool,
    ) -> ConductorHandle {
        NUM_CREATED.fetch_add(1, Ordering::SeqCst);

        Conductor::builder()
            .config(config.clone())
            .with_keystore(keystore)
            .no_print_setup()
            .test_builder_uses_production_k2_builder(test_builder_uses_production_k2_builder)
            .test(extra_dnas)
            .await
            .unwrap()
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn from_standard_config() -> SweetConductor {
        Self::from_config(SweetConductorConfig::standard()).await
    }

    /// Get the rendezvous config that this conductor is using, if any
    pub fn get_rendezvous_config(&self) -> Option<DynSweetRendezvous> {
        self.rendezvous.clone()
    }

    /// Access the database path for this conductor
    pub fn db_path(&self) -> &Path {
        &self.db_dir
    }

    /// Make the temp db dir persistent
    pub fn persist_dbs(&mut self) -> &Path {
        self.db_dir.persist();
        &self.db_dir
    }

    /// Access the MetaLairClient for this conductor
    pub fn keystore(&self) -> MetaLairClient {
        self.keystore.clone()
    }

    /// Convenience function that uses the internal handle to enable an app
    pub async fn enable_app(&self, id: InstalledAppId) -> ConductorResult<InstalledApp> {
        self.raw_handle().enable_app(id).await
    }

    /// Convenience function that uses the internal handle to disable an app
    pub async fn disable_app(
        &self,
        id: InstalledAppId,
        reason: DisabledAppReason,
    ) -> ConductorResult<InstalledApp> {
        self.raw_handle().disable_app(id, reason).await
    }

    /// Install the dna first.
    /// This allows a big speed up when
    /// installing many apps with the same dna
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn setup_app_1_register_dna(
        &mut self,
        dna_files: impl IntoIterator<Item = &DnaFile>,
    ) -> ConductorApiResult<()> {
        for dna_file in dna_files.into_iter() {
            self.register_dna(dna_file.to_owned()).await?;
            self.dnas.push(dna_file.to_owned());
        }
        Ok(())
    }

    /// Install the app and enable it
    // TODO: make this take a more flexible config for specifying things like
    // membrane proofs
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn setup_app_2_install_and_enable(
        &mut self,
        installed_app_id: &str,
        agent: Option<AgentPubKey>,
        dnas_with_roles: &[impl DnaWithRole],
    ) -> ConductorApiResult<AgentPubKey> {
        let installed_app_id = installed_app_id.to_string();

        let dnas_with_proof: Vec<_> = dnas_with_roles
            .iter()
            .cloned()
            .map(|dr| {
                let dna = dr.dna().clone().update_modifiers(Default::default());
                (dr.replace_dna(dna), None)
            })
            .collect();

        let agent = self
            .raw_handle()
            .install_app_minimal(installed_app_id.clone(), agent, &dnas_with_proof, None)
            .await?;

        self.raw_handle().enable_app(installed_app_id).await?;
        Ok(agent)
    }

    /// Build the SweetCells after `setup_cells` has been run
    /// The setup is split into two parts because the Cell environments
    /// are not available until after `setup_cells` has run, and it is
    /// better to do that once for all apps in the case of multiple apps being
    /// set up at once.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn setup_app_3_create_sweet_app(
        &self,
        installed_app_id: &str,
        agent: AgentPubKey,
        roles: &[RoleName],
    ) -> ConductorApiResult<SweetApp> {
        let info = self
            .raw_handle()
            .get_app_info(&installed_app_id.to_owned())
            .await
            .expect("Error getting AppInfo for just-installed app")
            .expect("Couldn't get AppInfo for just-installed app");

        let mut sweet_cells = Vec::new();

        for role in roles {
            if let Some(CellInfo::Provisioned(ProvisionedCell { cell_id, .. })) =
                info.cell_info[role].first()
            {
                assert_eq!(cell_id.agent_pubkey(), &agent, "Agent mismatch for cell");

                // Initialize per-space databases
                let _space = self.spaces.get_or_create_space(cell_id.dna_hash())?;

                // Create and add the SweetCell
                sweet_cells.push(self.get_sweet_cell(cell_id.clone())?);
            }
        }

        Ok(SweetApp::new(installed_app_id.into(), sweet_cells))
    }

    /// Construct a SweetCell for a cell which has already been created
    pub fn get_sweet_cell(&self, cell_id: CellId) -> ConductorApiResult<SweetCell> {
        let cell_authored_db = self
            .raw_handle()
            .get_or_create_authored_db(cell_id.dna_hash(), cell_id.agent_pubkey().clone())?;
        let cell_dht_db = self.raw_handle().get_dht_db(cell_id.dna_hash())?;
        let conductor_config = self.config.clone();
        Ok(SweetCell {
            cell_id,
            cell_authored_db,
            cell_dht_db,
            conductor_config,
        })
    }

    /// Opinionated app setup.
    /// Creates an app for the given agent, if specified, using the given DnaFiles,
    /// with no extra configuration.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn setup_app_for_optional_agent<'a>(
        &mut self,
        installed_app_id: &str,
        agent: Option<AgentPubKey>,
        dnas_with_roles: impl IntoIterator<Item = &'a (impl DnaWithRole + 'a)>,
    ) -> ConductorApiResult<SweetApp> {
        let dnas_with_roles: Vec<_> = dnas_with_roles.into_iter().cloned().collect();
        let dnas = dnas_with_roles
            .iter()
            .map(|dr| dr.dna())
            .collect::<Vec<_>>();

        self.setup_app_1_register_dna(dnas.clone()).await?;

        let agent = self
            .setup_app_2_install_and_enable(
                installed_app_id,
                agent.clone(),
                dnas_with_roles.as_slice(),
            )
            .await?;

        let roles = dnas_with_roles
            .iter()
            .map(|dr| dr.role())
            .collect::<Vec<_>>();
        self.setup_app_3_create_sweet_app(installed_app_id, agent, &roles)
            .await
    }

    /// Opinionated app setup.
    /// Creates an app for the given agent, using the given DnaFiles, with no extra configuration.
    pub async fn setup_app_for_agent<'a>(
        &mut self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dnas_with_roles: impl IntoIterator<Item = &'a (impl DnaWithRole + 'a)>,
    ) -> ConductorApiResult<SweetApp> {
        self.setup_app_for_optional_agent(installed_app_id, Some(agent), dnas_with_roles)
            .await
    }

    /// Opinionated app setup.
    /// Creates an app using the given DnaFiles, with no extra configuration.
    /// An AgentPubKey will be generated, and is accessible via the returned SweetApp.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub async fn setup_app<'a>(
        &mut self,
        installed_app_id: &str,
        dnas_with_roles: impl IntoIterator<Item = &'a (impl DnaWithRole + 'a)> + Clone,
    ) -> ConductorApiResult<SweetApp> {
        self.setup_app_for_optional_agent(installed_app_id, None, dnas_with_roles)
            .await
    }

    /// Opinionated app setup. Creates one app per agent, using the given DnaFiles.
    ///
    /// All InstalledAppIds and RoleNames are auto-generated. In tests driven directly
    /// by Rust, you typically won't care what these values are set to, but in case you
    /// do, they are set as so:
    /// - InstalledAppId: {app_id_prefix}-{agent_pub_key}
    /// - RoleName: {dna_hash}
    ///
    /// Returns a batch of SweetApps, sorted in the same order as Agents passed in.
    pub async fn setup_app_for_agents<'a>(
        &mut self,
        app_id_prefix: &str,
        agents: impl IntoIterator<Item = &AgentPubKey>,
        dnas_with_roles: impl IntoIterator<Item = &'a (impl DnaWithRole + 'a)>,
    ) -> ConductorApiResult<SweetAppBatch> {
        let agents: Vec<_> = agents.into_iter().collect();
        let dnas_with_roles: Vec<_> = dnas_with_roles.into_iter().cloned().collect();
        let dnas: Vec<&DnaFile> = dnas_with_roles.iter().map(|dr| dr.dna()).collect();
        let roles: Vec<RoleName> = dnas_with_roles.iter().map(|dr| dr.role()).collect();
        self.setup_app_1_register_dna(dnas.clone()).await?;

        for &agent in agents.iter() {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            self.setup_app_2_install_and_enable(
                &installed_app_id,
                Some(agent.to_owned()),
                &dnas_with_roles,
            )
            .await?;
        }

        let mut apps = Vec::new();
        for agent in agents {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            apps.push(
                self.setup_app_3_create_sweet_app(&installed_app_id, agent.clone(), &roles)
                    .await?,
            );
        }

        Ok(SweetAppBatch(apps))
    }

    /// Setup N apps with generated agent keys and the same set of DNAs
    pub async fn setup_apps<'a>(
        &mut self,
        app_id_prefix: &str,
        num: usize,
        dnas_with_roles: impl IntoIterator<Item = &'a (impl DnaWithRole + 'a)>,
    ) -> ConductorApiResult<SweetAppBatch> {
        let dnas_with_roles: Vec<_> = dnas_with_roles.into_iter().cloned().collect();

        let mut apps = vec![];

        for i in 0..num {
            let app = self
                .setup_app(&format!("{}{}", app_id_prefix, i), &dnas_with_roles)
                .await?;
            apps.push(app);
        }

        Ok(SweetAppBatch(apps))
    }

    /// Call into the underlying create_clone_cell function, and register the
    /// created dna with SweetConductor so it will be reloaded on restart.
    pub async fn create_clone_cell(
        &mut self,
        installed_app_id: &InstalledAppId,
        payload: CreateCloneCellPayload,
    ) -> ConductorResult<holochain_zome_types::clone::ClonedCell> {
        let clone = self
            .raw_handle()
            .create_clone_cell(installed_app_id, payload)
            .await?;
        let dna_file = self.get_dna_file(clone.cell_id.dna_hash()).unwrap();
        self.dnas.push(dna_file);
        Ok(clone)
    }

    /// Get a new websocket client which can send requests over the admin
    /// interface. It presupposes that an admin interface has been configured.
    /// (The standard_config includes an admin interface at port 0.)
    pub async fn admin_ws_client<D>(&self) -> (WebsocketSender, WsPollRecv)
    where
        D: std::fmt::Debug,
        SerializedBytes: TryInto<D, Error = SerializedBytesError>,
    {
        let port = self
            .get_arbitrary_admin_websocket_port()
            .expect("No admin port open on conductor");
        let (tx, rx) = websocket_client_by_port(port).await.unwrap();

        (tx, WsPollRecv::new::<D>(rx))
    }

    /// Create a new app interface and get a websocket client which can send requests
    /// to it.
    pub async fn app_ws_client<D>(
        &self,
        installed_app_id: InstalledAppId,
    ) -> (WebsocketSender, WsPollRecv)
    where
        D: std::fmt::Debug,
        SerializedBytes: TryInto<D, Error = SerializedBytesError>,
    {
        let port = self
            .raw_handle()
            .add_app_interface(either::Either::Left(0), AllowedOrigins::Any, None)
            .await
            .expect("Couldn't create app interface");
        let (tx, rx) = websocket_client_by_port(port).await.unwrap();

        authenticate_app_ws_client(
            tx.clone(),
            self.get_arbitrary_admin_websocket_port()
                .expect("No admin ports on this conductor"),
            installed_app_id,
        )
        .await;

        (tx, WsPollRecv::new::<D>(rx))
    }

    /// Shutdown this conductor.
    /// This will wait for the conductor to shut down but
    /// keep the inner state to restart it.
    ///
    /// Attempting to use this conductor without starting it up again will cause a panic.
    pub async fn shutdown(&mut self) {
        self.try_shutdown().await.unwrap();
    }

    /// Shutdown this conductor.
    /// This will wait for the conductor to shutdown but
    /// keep the inner state to restart it.
    ///
    /// Attempting to use this conductor without starting it up again will cause a panic.
    pub async fn try_shutdown(&mut self) -> std::io::Result<()> {
        if let Some(handle) = self.handle.take() {
            handle
                .shutdown()
                .await
                .map_err(Error::other)?
                .map_err(Error::other)
        } else {
            panic!("Attempted to shutdown conductor which was already shutdown");
        }
    }

    /// Start up this conductor if it's not already running.
    pub async fn startup(&mut self) {
        if self.handle.is_none() {
            // There's a db dir in the sweet conductor and the config, that are
            // supposed to be the same. Let's assert that they are.
            assert_eq!(
                Some(self.db_dir.as_ref().to_path_buf().into()),
                self.config.data_root_path,
                "SweetConductor db_dir and config.data_root_path are not the same",
            );
            self.handle = Some(SweetConductorHandle(
                Self::handle_from_existing(
                    self.keystore.clone(),
                    &self.config,
                    self.dnas.as_slice(),
                    false,
                )
                .await,
            ));
        } else {
            panic!("Attempted to start conductor which was already started");
        }
    }

    /// Check if this conductor is running
    pub fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    /// Get the underlying SweetConductorHandle.
    #[allow(dead_code)]
    pub fn sweet_handle(&self) -> SweetConductorHandle {
        self.handle
            .as_ref()
            .map(|h| h.clone_privately())
            .expect("Tried to use a conductor that is offline")
    }

    /// Get the ConductorHandle within this Conductor.
    /// Be careful when using this, because this leaks out handles, which may
    /// make it harder to shut down the conductor during tests.
    pub fn raw_handle(&self) -> ConductorHandle {
        self.handle
            .as_ref()
            .map(|h| h.0.clone())
            .expect("Tried to use a conductor that is offline")
    }

    /// Let each conductor know about each other's agents so they can do networking.
    ///
    /// Returns a boolean indicating whether each space has at least one agent info for each conductor.
    pub async fn exchange_peer_info(conductors: impl Clone + IntoIterator<Item = &Self>) -> bool {
        // Combined peer info set across all conductors, separated by DNA hash (space)
        let mut all = HashMap::<Arc<DnaHash>, HashSet<_>>::new();

        let conductor_count = conductors.clone().into_iter().count();

        // Collect all the agent infos across the spaces on these conductors.
        for c in conductors.clone().into_iter() {
            if c.get_config().has_rendezvous_bootstrap() {
                panic!(
                    "exchange_peer_info cannot reliably be used with rendezvous bootstrap servers"
                );
            }

            for dna_hash in c.spaces.get_from_spaces(|s| s.dna_hash.clone()) {
                let agent_infos = c
                    .holochain_p2p()
                    .peer_store((*dna_hash).clone())
                    .await
                    .unwrap()
                    .get_all()
                    .await
                    .unwrap();

                all.entry(dna_hash).or_default().extend(agent_infos);
            }
        }

        // Insert the agent infos into each conductor's peer store
        for c in conductors.into_iter() {
            for dna_hash in c.spaces.get_from_spaces(|s| s.dna_hash.clone()) {
                let inject_agent_infos = all.get(&dna_hash).unwrap().iter().cloned().collect();
                tracing::info!("Injecting agent infos: {:?}", inject_agent_infos);
                c.holochain_p2p()
                    .peer_store((*dna_hash).clone())
                    .await
                    .unwrap()
                    .insert(inject_agent_infos)
                    .await
                    .unwrap();
            }
        }

        // Check that each space has at least one agent info for each conductor
        all.iter().all(|(_, v)| v.len() >= conductor_count)
    }

    /// Wait for at least one gossip round to have completed for the given cell
    ///
    /// Note that this is really a crutch. If gossip starts fast enough then this is unnecessary
    /// but that doesn't necessarily happen. Waiting for gossip to have started before, for example,
    /// waiting for something else like consistency is useful to ensure that communication has
    /// actually started.
    pub async fn require_initial_gossip_activity_for_cell(
        &self,
        cell: &SweetCell,
        min_peers: u32,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let handle = self.raw_handle();

        let wait_start = Instant::now();
        loop {
            let (number_of_peers, completed_rounds) = handle
                .dump_network_metrics(Kitsune2NetworkMetricsRequest {
                    dna_hash: Some(cell.cell_id().dna_hash().clone()),
                    ..Default::default()
                })
                .await?
                .get(cell.cell_id.dna_hash())
                .map_or((0, 0), |info| {
                    (
                        // The number of peers we're holding metadata for
                        info.gossip_state_summary.peer_meta.len(),
                        info.gossip_state_summary
                            .peer_meta
                            .values()
                            .map(|meta| meta.completed_rounds.unwrap_or_default())
                            .sum(),
                    )
                });

            if number_of_peers >= min_peers as usize && completed_rounds > 0 {
                tracing::info!(
                    "Took {}s for cell {} to complete {} gossip rounds",
                    wait_start.elapsed().as_secs(),
                    cell.cell_id(),
                    completed_rounds
                );
                return Ok(());
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            if wait_start.elapsed() > timeout {
                anyhow::bail!(
                    "Timed out waiting for gossip to start for cell {}",
                    cell.cell_id()
                );
            }
        }
    }

    /// Instantiate a source chain object for the given agent and DNA hash.
    pub async fn get_agent_source_chain(
        &self,
        agent_key: &AgentPubKey,
        dna_hash: &DnaHash,
    ) -> SourceChain {
        SourceChain::new(
            self.get_or_create_authored_db(dna_hash, agent_key.clone())
                .unwrap(),
            self.get_dht_db(dna_hash).unwrap(),
            self.keystore().clone(),
            agent_key.clone(),
        )
        .await
        .unwrap()
    }

    /// Retries getting a list of peers from the conductor until all the given peers are in the response.
    ///
    /// You can optionally filter by `cell_id`. That is used in the `get_agent_infos` call to the conductor, so you
    /// can see how that works in the conductor docs.
    ///
    /// If the max_wait is reached then this function will return a "Timeout" error.
    pub async fn wait_for_peer_visible<P: IntoIterator<Item = AgentPubKey>>(
        &self,
        peers: P,
        cell_id: Option<CellId>,
        max_wait: Duration,
    ) -> ConductorApiResult<()> {
        let handle = self.raw_handle();

        let peers = peers.into_iter().collect::<HashSet<_>>();

        tokio::time::timeout(max_wait, async move {
            loop {
                let infos = handle
                    .get_agent_infos(
                        cell_id
                            .clone()
                            .map(|cell_id| vec![cell_id.dna_hash().clone()]),
                    )
                    .await?
                    .into_iter()
                    .map(|p| AgentPubKey::from_k2_agent(&p.agent))
                    .collect::<HashSet<_>>();
                if infos.is_superset(&peers) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            Ok(())
        })
        .await
        .map_err(|_| ConductorApiError::other("Timeout"))?
    }

    /// Declare full storage arc for all agents of the space and wait until the
    /// agent infos have been published to the peer store.
    ///
    /// # Panics
    ///
    /// If peer store cannot be found for DNA hash.
    /// If publishing to the peer store fails within the timeout of 5 s.
    pub async fn declare_full_storage_arcs(&self, dna_hash: &DnaHash) {
        self.holochain_p2p()
            .test_set_full_arcs(dna_hash.to_k2_space())
            .await;
        let local_agents = self
            .holochain_p2p()
            .test_kitsune()
            .space(dna_hash.to_k2_space())
            .await
            .unwrap()
            .local_agent_store()
            .get_all()
            .await
            .unwrap()
            .into_iter()
            .map(|agent| agent.agent().clone())
            .collect::<Vec<_>>();
        let peer_store = self
            .holochain_p2p()
            .peer_store(dna_hash.clone())
            .await
            .unwrap();
        retry_until_timeout!(5_000, 500, {
            if peer_store
                .get_all()
                .await
                .unwrap()
                .into_iter()
                // Only check this conductor's local agents for full storage arc.
                .filter(|agent_info| local_agents.contains(&agent_info.agent))
                .all(|agent_info| agent_info.storage_arc == DhtArc::FULL)
            {
                break;
            }
        });
    }

    /// Getter
    pub fn rendezvous(&self) -> Option<&DynSweetRendezvous> {
        self.rendezvous.as_ref()
    }

    /// Check if all ops in the DHT database have been integrated.
    pub fn all_ops_integrated(&self, dna_hash: &DnaHash) -> ConductorApiResult<bool> {
        let dht_db = self.get_dht_db(dna_hash)?;
        dht_db.test_read(|txn| {
            let all_integrated = txn
                .query_row(
                    "SELECT NOT EXISTS(SELECT 1 FROM DhtOp WHERE when_integrated IS NULL)",
                    [],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap();
            Ok(all_integrated)
        })
    }

    /// Check if all ops of a specific author have been integrated in the DHT database.
    pub fn all_ops_of_author_integrated(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> ConductorApiResult<bool> {
        let dht_db = self.get_dht_db(dna_hash)?;
        let author = author.clone();
        dht_db.test_read(move |txn| {
            let all_integrated = txn
                .query_row(
                    "SELECT NOT EXISTS(
                            SELECT 1
                            FROM DhtOp
                            JOIN Action
                            ON Action.hash = DhtOp.action_hash
                            WHERE Action.author = :author
                            AND DhtOp.when_integrated IS NULL
                        )",
                    named_params! {":author": author},
                    |row| row.get::<_, bool>(0),
                )
                .unwrap();
            Ok(all_integrated)
        })
    }
}

/// You do not need to do anything with this type. While it is held it will keep polling a websocket
/// receiver.
pub struct WsPollRecv(tokio::task::JoinHandle<()>);

impl Drop for WsPollRecv {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl WsPollRecv {
    /// Create a new [WsPollRecv] that will poll the given [WebsocketReceiver] for messages.
    /// The type of the messages being received must be specified. For example
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()>
    /// # {
    ///
    /// use holochain::sweettest::{websocket_client_by_port, WsPollRecv};
    /// use holochain_conductor_api::AdminResponse;
    ///
    /// let (tx, rx) = websocket_client_by_port(3000).await?;
    /// let _rx = WsPollRecv::new::<AdminResponse>(rx);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<D>(mut rx: WebsocketReceiver) -> Self
    where
        D: std::fmt::Debug,
        SerializedBytes: TryInto<D, Error = SerializedBytesError>,
    {
        Self(tokio::task::spawn(async move {
            while rx.recv::<D>().await.is_ok() {}
        }))
    }
}

/// Connect to a websocket server at the given port.
///
/// Note that the [WebsocketReceiver] returned by this function will need to be polled. This can be
/// done with a [WsPollRecv].
/// If this is an app client, you will need to authenticate the connection before you can send any
/// other requests.
pub async fn websocket_client_by_port(
    port: u16,
) -> WebsocketResult<(WebsocketSender, WebsocketReceiver)> {
    connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{port}")
                .to_socket_addrs()?
                .next()
                .ok_or_else(|| Error::other("Could not resolve localhost"))?,
        ),
    )
    .await
}

/// Create an authentication token for an app client and authenticate the connection.
pub async fn authenticate_app_ws_client(
    app_sender: WebsocketSender,
    admin_port: u16,
    installed_app_id: InstalledAppId,
) {
    let (admin_tx, admin_rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _admin_rx = WsPollRecv::new::<AdminResponse>(admin_rx);

    let token_response: AdminResponse = admin_tx
        .request(AdminRequest::IssueAppAuthenticationToken(
            installed_app_id.into(),
        ))
        .await
        .unwrap();
    let token = match token_response {
        AdminResponse::AppAuthenticationTokenIssued(issued) => issued.token,
        _ => panic!("unexpected response"),
    };

    app_sender
        .authenticate(AppAuthenticationRequest { token })
        .await
        .unwrap();
}

impl Drop for SweetConductor {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            tokio::task::spawn(handle.shutdown());
        }
    }
}

impl AsRef<SweetConductorHandle> for SweetConductor {
    fn as_ref(&self) -> &SweetConductorHandle {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}

impl std::ops::Deref for SweetConductor {
    type Target = SweetConductorHandle;

    fn deref(&self) -> &Self::Target {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}

impl std::borrow::Borrow<SweetConductorHandle> for SweetConductor {
    fn borrow(&self) -> &SweetConductorHandle {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}

impl std::fmt::Debug for SweetConductor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SweetConductor")
            .field("db_dir", &self.db_dir)
            .field("config", &self.config)
            .field("dnas", &self.dnas)
            .finish()
    }
}

#[allow(dead_code)]
fn covering(rng: &mut StdRng, n: usize, s: usize) -> Vec<HashSet<usize>> {
    let nodes: Vec<_> = (0..n)
        .map(|i| {
            let peers: HashSet<_> = std::iter::repeat_with(|| rng.random_range(0..n))
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
