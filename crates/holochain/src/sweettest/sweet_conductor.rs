//! A wrapper around ConductorHandle with more convenient methods for testing
// TODO [ B-03669 ] move to own crate

use super::{
    DynSweetRendezvous, SweetAgents, SweetApp, SweetAppBatch, SweetCell, SweetConductorConfig,
    SweetConductorHandle, SweetLocalRendezvous,
};
use crate::conductor::state::AppInterfaceId;
use crate::conductor::ConductorHandle;
use crate::conductor::{
    api::error::ConductorApiResult, config::ConductorConfig, error::ConductorResult, space::Spaces,
    CellError, Conductor, ConductorBuilder,
};
use ::fixt::prelude::StdRng;
use hdk::prelude::*;
use holo_hash::DnaHash;
use holochain_keystore::MetaLairClient;
use holochain_state::prelude::test_db_dir;
use holochain_state::test_utils::TestDir;
use holochain_types::prelude::*;
use holochain_websocket::*;
use rand::Rng;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

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
    pub(crate) spaces: Spaces,
    config: ConductorConfig,
    dnas: Vec<DnaFile>,
    signal_stream: Option<SignalStream>,
    _rendezvous: Option<DynSweetRendezvous>,
}

/// Standard config for SweetConductors
pub fn standard_config() -> SweetConductorConfig {
    SweetConductorConfig::standard()
}

/// A DnaFile with a role name assigned
#[derive(Clone)]
pub struct DnaWithRole {
    role: RoleName,
    dna: DnaFile,
}

impl From<DnaFile> for DnaWithRole {
    fn from(dna: DnaFile) -> Self {
        Self {
            // Assign a dummy unique throwaway role
            role: format!("{}", dna.dna_hash()),
            dna,
        }
    }
}

impl From<(RoleName, DnaFile)> for DnaWithRole {
    fn from((role, dna): (RoleName, DnaFile)) -> Self {
        Self { role, dna }
    }
}

impl SweetConductor {
    /// Create a SweetConductor from an already-built ConductorHandle and environments
    /// RibosomeStore
    /// The conductor will be supplied with a single test AppInterface named
    /// "sweet-interface" so that signals may be emitted
    pub async fn new(
        handle: ConductorHandle,
        env_dir: TestDir,
        config: ConductorConfig,
        _rendezvous: Option<DynSweetRendezvous>,
    ) -> SweetConductor {
        // Automatically add a test app interface
        handle
            .add_test_app_interface(AppInterfaceId::default())
            .await
            .expect("Couldn't set up test app interface");

        // Get a stream of all signals since conductor startup
        let signal_stream = handle.signal_broadcaster().subscribe_merged();

        // XXX: this is a bit wonky.
        // We create a Spaces instance here purely because it's easier to initialize
        // the per-space databases this way. However, we actually use the TestEnvs
        // to actually access those databases.
        // As a TODO, we can remove the need for TestEnvs in sweettest or have
        // some other better integration between the two.
        let spaces = Spaces::new(&ConductorConfig {
            environment_path: env_dir.to_path_buf().into(),
            ..Default::default()
        })
        .unwrap();

        let keystore = handle.keystore().clone();

        Self {
            handle: Some(SweetConductorHandle(handle)),
            db_dir: env_dir,
            keystore,
            spaces,
            config,
            dnas: Vec::new(),
            signal_stream: Some(Box::new(signal_stream)),
            _rendezvous,
        }
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn from_config<C>(config: C) -> SweetConductor
    where
        C: Into<SweetConductorConfig>,
    {
        let rendezvous = SweetLocalRendezvous::new().await;
        Self::from_config_rendezvous(config, rendezvous).await
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn from_config_rendezvous<C, R>(config: C, rendezvous: R) -> SweetConductor
    where
        C: Into<SweetConductorConfig>,
        R: Into<DynSweetRendezvous>,
    {
        let keystore = test_keystore();
        Self::from_config_rendezvous_keystore(config, rendezvous, keystore).await
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn from_config_rendezvous_keystore<C, R>(
        config: C,
        rendezvous: R,
        keystore: holochain_keystore::MetaLairClient,
    ) -> SweetConductor
    where
        C: Into<SweetConductorConfig>,
        R: Into<DynSweetRendezvous>,
    {
        let rendezvous = rendezvous.into();
        let config = config.into().into_conductor_config(&*rendezvous).await;
        tracing::info!(?config);
        let dir = TestDir::new(test_db_dir());
        assert!(
            dir.read_dir().unwrap().next().is_none(),
            "Test dir not empty - {:?}",
            dir.to_path_buf()
        );
        let handle = Self::handle_from_existing(&dir, keystore, &config, &[]).await;
        Self::new(handle, dir, config, Some(rendezvous)).await
    }

    /// Create a SweetConductor from a partially-configured ConductorBuilder
    pub async fn from_builder(builder: ConductorBuilder) -> SweetConductor {
        let db_dir = TestDir::new(test_db_dir());
        let config = builder.config.clone();
        let handle = builder.test(&db_dir, &[]).await.unwrap();
        Self::new(handle, db_dir, config, None).await
    }

    /// Create a handle from an existing environment and config
    pub async fn handle_from_existing(
        db_dir: &Path,
        keystore: MetaLairClient,
        config: &ConductorConfig,
        extra_dnas: &[DnaFile],
    ) -> ConductorHandle {
        Conductor::builder()
            .config(config.clone())
            .with_keystore(keystore)
            .no_print_setup()
            .test(db_dir, extra_dnas)
            .await
            .unwrap()
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn from_standard_config() -> SweetConductor {
        Self::from_config(standard_config()).await
    }

    /// Get the rendezvous config that this conductor is using, if any
    pub fn get_rendezvous_config(&self) -> Option<DynSweetRendezvous> {
        self._rendezvous.clone()
    }

    /// Access the database path for this conductor
    pub fn db_path(&self) -> &Path {
        &self.db_dir
    }

    /// Make the temp db dir persistent
    pub fn persist(&mut self) -> &Path {
        self.db_dir.persist();
        &self.db_dir
    }

    /// Access the MetaLairClient for this conductor
    pub fn keystore(&self) -> MetaLairClient {
        self.keystore.clone()
    }

    /// Convenience function that uses the internal handle to enable an app
    pub async fn enable_app(
        &self,
        id: InstalledAppId,
    ) -> ConductorResult<(InstalledApp, Vec<(CellId, CellError)>)> {
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

    /// Convenience function that uses the internal handle to start an app
    pub async fn start_app(&self, id: InstalledAppId) -> ConductorResult<InstalledApp> {
        self.raw_handle().start_app(id).await
    }

    /// Convenience function that uses the internal handle to pause an app
    pub async fn pause_app(
        &self,
        id: InstalledAppId,
        reason: PausedAppReason,
    ) -> ConductorResult<InstalledApp> {
        self.raw_handle().pause_app(id, reason).await
    }

    /// Install the dna first.
    /// This allows a big speed up when
    /// installing many apps with the same dna
    async fn setup_app_1_register_dna(&mut self, dna_files: &[&DnaFile]) -> ConductorApiResult<()> {
        for &dna_file in dna_files {
            self.register_dna(dna_file.clone()).await?;
            self.dnas.push(dna_file.clone());
        }
        Ok(())
    }

    /// Install the app and enable it
    // TODO: make this take a more flexible config for specifying things like
    // membrane proofs
    async fn setup_app_2_install_and_enable(
        &mut self,
        installed_app_id: &str,
        agent: AgentPubKey,
        roles: &[DnaWithRole],
    ) -> ConductorApiResult<()> {
        let installed_app_id = installed_app_id.to_string();

        let installed_cells = roles
            .iter()
            .map(|r| {
                let cell_id = CellId::new(r.dna.dna_hash().clone(), agent.clone());
                (InstalledCell::new(cell_id, r.role.clone()), None)
            })
            .collect();
        self.raw_handle()
            .install_app_legacy(installed_app_id.clone(), installed_cells)
            .await?;

        self.raw_handle().enable_app(installed_app_id).await?;
        Ok(())
    }

    /// Build the SweetCells after `setup_cells` has been run
    /// The setup is split into two parts because the Cell environments
    /// are not available until after `setup_cells` has run, and it is
    /// better to do that once for all apps in the case of multiple apps being
    /// set up at once.
    async fn setup_app_3_create_sweet_app(
        &self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_hashes: impl Iterator<Item = DnaHash>,
    ) -> ConductorApiResult<SweetApp> {
        let mut sweet_cells = Vec::new();
        for dna_hash in dna_hashes {
            // Initialize per-space databases
            let _space = self.spaces.get_or_create_space(&dna_hash)?;

            // Create and add the SweetCell
            sweet_cells.push(self.get_sweet_cell(CellId::new(dna_hash, agent.clone()))?);
        }

        Ok(SweetApp::new(installed_app_id.into(), sweet_cells))
    }

    /// Construct a SweetCell for a cell which has already been created
    pub fn get_sweet_cell(&self, cell_id: CellId) -> ConductorApiResult<SweetCell> {
        let (dna_hash, agent) = cell_id.into_dna_and_agent();
        let cell_authored_db = self.raw_handle().get_authored_db(&dna_hash)?;
        let cell_dht_db = self.raw_handle().get_dht_db(&dna_hash)?;
        let cell_id = CellId::new(dna_hash, agent);
        Ok(SweetCell {
            cell_id,
            cell_authored_db,
            cell_dht_db,
        })
    }

    /// Opinionated app setup.
    /// Creates an app for the given agent, using the given DnaFiles, with no extra configuration.
    pub async fn setup_app_for_agent<'a, R, D>(
        &mut self,
        installed_app_id: &str,
        agent: AgentPubKey,
        roles: D,
    ) -> ConductorApiResult<SweetApp>
    where
        R: Into<DnaWithRole> + Clone + 'a,
        D: IntoIterator<Item = &'a R>,
    {
        let roles: Vec<DnaWithRole> = roles.into_iter().cloned().map(Into::into).collect();
        let dnas = roles.iter().map(|r| &r.dna).collect::<Vec<_>>();
        self.setup_app_1_register_dna(&dnas).await?;
        self.setup_app_2_install_and_enable(installed_app_id, agent.clone(), roles.as_slice())
            .await?;

        self.raw_handle()
            .reconcile_cell_status_with_app_status()
            .await?;

        let dna_hashes = roles.iter().map(|r| r.dna.dna_hash().clone());
        self.setup_app_3_create_sweet_app(installed_app_id, agent, dna_hashes)
            .await
    }

    /// Opinionated app setup.
    /// Creates an app using the given DnaFiles, with no extra configuration.
    /// An AgentPubKey will be generated, and is accessible via the returned SweetApp.
    pub async fn setup_app<'a, R, D>(
        &mut self,
        installed_app_id: &str,
        dnas: D,
    ) -> ConductorApiResult<SweetApp>
    where
        R: Into<DnaWithRole> + Clone + 'a,
        D: IntoIterator<Item = &'a R> + Clone,
    {
        let agent = SweetAgents::one(self.keystore()).await;
        self.setup_app_for_agent(installed_app_id, agent, dnas.clone())
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
    pub async fn setup_app_for_agents<'a, A, R, D>(
        &mut self,
        app_id_prefix: &str,
        agents: A,
        roles: D,
    ) -> ConductorApiResult<SweetAppBatch>
    where
        A: IntoIterator<Item = &'a AgentPubKey>,
        R: Into<DnaWithRole> + Clone + 'a,
        D: IntoIterator<Item = &'a R>,
    {
        let agents: Vec<_> = agents.into_iter().collect();
        let roles: Vec<DnaWithRole> = roles.into_iter().cloned().map(Into::into).collect();
        let dnas: Vec<&DnaFile> = roles.iter().map(|r| &r.dna).collect();
        self.setup_app_1_register_dna(dnas.as_slice()).await?;
        for &agent in agents.iter() {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            self.setup_app_2_install_and_enable(
                &installed_app_id,
                agent.to_owned(),
                roles.as_slice(),
            )
            .await?;
        }

        self.raw_handle()
            .reconcile_cell_status_with_app_status()
            .await?;

        let mut apps = Vec::new();
        for agent in agents {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            apps.push(
                self.setup_app_3_create_sweet_app(
                    &installed_app_id,
                    agent.clone(),
                    roles.iter().map(|r| r.dna.dna_hash().clone()),
                )
                .await?,
            );
        }

        Ok(SweetAppBatch(apps))
    }

    /// Call into the underlying create_clone_cell function, and register the
    /// created dna with SweetConductor so it will be reloaded on restart.
    pub async fn create_clone_cell(
        &mut self,
        payload: CreateCloneCellPayload,
    ) -> ConductorApiResult<holochain_conductor_api::ClonedCell> {
        let clone = self.raw_handle().create_clone_cell(payload).await?;
        let dna_file = self.get_dna_file(clone.cell_id.dna_hash()).unwrap();
        self.dnas.push(dna_file);
        Ok(clone)
    }

    /// Get a stream of all Signals emitted on the "sweet-interface" AppInterface.
    ///
    /// This is designed to crash if called more than once, because as currently
    /// implemented, creating multiple signal streams would simply cause multiple
    /// consumers of the same underlying streams, not a fresh subscription
    pub fn signals(&mut self) -> SignalStream {
        self.signal_stream
            .take()
            .expect("Can't take the SweetConductor signal stream twice")
    }

    /// Get a new websocket client which can send requests over the admin
    /// interface. It presupposes that an admin interface has been configured.
    /// (The standard_config includes an admin interface at port 0.)
    pub async fn admin_ws_client(&self) -> (WebsocketSender, WebsocketReceiver) {
        let port = self
            .get_arbitrary_admin_websocket_port()
            .expect("No admin port open on conductor");
        websocket_client_by_port(port).await.unwrap()
    }

    /// Shutdown this conductor.
    /// This will wait for the conductor to shutdown but
    /// keep the inner state to restart it.
    ///
    /// Attempting to use this conductor without starting it up again will cause a panic.
    pub async fn shutdown(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.shutdown().await.unwrap().unwrap();
        } else {
            panic!("Attempted to shutdown conductor which was already shutdown");
        }
    }

    /// Start up this conductor if it's not already running.
    pub async fn startup(&mut self) {
        if self.handle.is_none() {
            self.handle = Some(SweetConductorHandle(
                Self::handle_from_existing(
                    &self.db_dir,
                    self.keystore.clone(),
                    &self.config,
                    self.dnas.as_slice(),
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

    /// Force trigger all dht ops that haven't received
    /// enough validation receipts yet.
    pub async fn force_all_publish_dht_ops(&self) {
        use futures::stream::StreamExt;
        if let Some(handle) = self.handle.as_ref() {
            let iter = handle.running_cell_ids(None).into_iter().map(|id| async {
                let id = id;
                let db = self.get_authored_db(id.dna_hash()).unwrap();
                let trigger = self.get_cell_triggers(&id).await.unwrap();
                (db, trigger)
            });
            futures::stream::iter(iter)
                .then(|f| f)
                .for_each(|(db, mut triggers)| async move {
                    // The line below was added when migrating to rust edition 2021, per
                    // https://doc.rust-lang.org/edition-guide/rust-2021/disjoint-capture-in-closures.html#migration
                    let _ = &triggers;
                    crate::test_utils::force_publish_dht_ops(&db, &mut triggers.publish_dht_ops)
                        .await
                        .unwrap();
                })
                .await;
        }
    }

    /// Let each conductor know about each others' agents so they can do networking
    pub async fn exchange_peer_info(conductors: impl IntoIterator<Item = &Self>) {
        let mut all = Vec::new();
        for c in conductors.into_iter() {
            for env in c.spaces.get_from_spaces(|s| s.p2p_agents_db.clone()) {
                all.push(env.clone());
            }
        }
        crate::conductor::p2p_agent_store::exchange_peer_info(all).await;
    }

    /// Let each conductor know about each others' agents so they can do networking
    pub async fn exchange_peer_info_sampled(
        conductors: impl IntoIterator<Item = &Self>,
        rng: &mut StdRng,
        s: usize,
    ) {
        let mut all = Vec::new();
        for c in conductors.into_iter() {
            for env in c.spaces.get_from_spaces(|s| s.p2p_agents_db.clone()) {
                all.push(env.clone());
            }
        }
        let connectivity = covering(rng, all.len(), s);
        crate::conductor::p2p_agent_store::exchange_peer_info_sparse(all, connectivity).await;
    }

    /// Wait for at least one gossip round to have completed for the given cell
    ///
    /// Note that this is really a crutch. If gossip starts fast enough then this is unnecessary
    /// but that doesn't necessarily happen. Waiting for gossip to have started before, for example,
    /// waiting for something else like consistency is useful to ensure that communication has
    /// actually started.
    pub async fn require_initial_gossip_activity_for_cell(&self, cell: &SweetCell) {
        let handle = self.raw_handle();

        let wait_start = Instant::now();
        loop {
            let completed_rounds = handle
                .network_info(&NetworkInfoRequestPayload {
                    agent_pub_key: cell.agent_pubkey().clone(),
                    dnas: vec![cell.cell_id.dna_hash().clone()],
                    last_time_queried: None, // Just care about seeing the first data
                })
                .await
                .expect("Could not get network info")
                .first()
                .map_or(0, |info| info.completed_rounds_since_last_time_queried);

            if completed_rounds > 0 {
                tracing::info!(
                    "Took {}s for cell {} to complete {} gossip rounds",
                    wait_start.elapsed().as_secs(),
                    cell.cell_id(),
                    completed_rounds
                );
                return;
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
}

/// Get a websocket client on localhost at the specified port
pub async fn websocket_client_by_port(
    port: u16,
) -> WebsocketResult<(WebsocketSender, WebsocketReceiver)> {
    holochain_websocket::connect(
        url2::url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await
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
