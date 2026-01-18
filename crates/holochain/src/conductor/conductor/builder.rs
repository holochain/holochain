use super::*;
use crate::conductor::manager::OutcomeReceiver;
use crate::conductor::metrics::{create_post_commit_duration_metric, PostCommitDurationMetric};
use crate::conductor::paths::DataRootPath;
use crate::conductor::ribosome_store::RibosomeStore;
use crate::conductor::ConductorHandle;
use holochain_conductor_api::conductor::paths::KeystorePath;
use holochain_p2p::NetworkCompatParams;
use lair_keystore_api::types::SharedLockedArray;
use std::sync::Mutex;

/// A configurable Builder for Conductor and sometimes ConductorHandle
#[derive(Default)]
pub struct ConductorBuilder {
    /// The configuration
    pub config: ConductorConfig,

    /// The RibosomeStore (mockable)
    pub ribosome_store: RibosomeStore,

    /// For new lair, passphrase is required
    pub passphrase: Option<SharedLockedArray>,

    /// Optional keystore override
    pub keystore: Option<MetaLairClient>,

    /// Skip printing setup info to stdout
    pub no_print_setup: bool,

    /// WARNING!! DANGER!! This exposes your database decryption secrets!
    /// Print the database decryption secrets to stderr.
    /// With these PRAGMA commands, you'll be able to run sqlcipher
    /// directly to manipulate holochain databases.
    pub danger_print_db_secrets: bool,
}

impl ConductorBuilder {
    /// Default ConductorBuilder.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ConductorBuilder {
    /// Set the ConductorConfig used to build this Conductor
    pub fn config(mut self, config: ConductorConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the passphrase for use in keystore initialization
    pub fn passphrase(mut self, passphrase: Option<SharedLockedArray>) -> Self {
        self.passphrase = passphrase;
        self
    }

    /// Set up the builder to skip printing setup
    pub fn no_print_setup(mut self) -> Self {
        self.no_print_setup = true;
        self
    }

    /// WARNING!! DANGER!! This exposes your database decryption secrets!
    /// Print the database decryption secrets to stderr.
    /// With these PRAGMA commands, you'll be able to run sqlcipher
    /// directly to manipulate holochain databases.
    pub fn danger_print_db_secrets(mut self, v: bool) -> Self {
        self.danger_print_db_secrets = v;
        self
    }

    /// Set the data root path for the conductor that will be built.
    pub fn with_data_root_path(mut self, data_root_path: DataRootPath) -> Self {
        self.config.data_root_path = Some(data_root_path);
        self
    }

    /// Initialize a "production" Conductor
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(scope = self.config.network.tracing_scope)))]
    pub async fn build(self) -> ConductorResult<ConductorHandle> {
        let builder = self;
        tracing::debug!(?builder.config);

        if builder
            .config
            .tuning_params
            .as_ref()
            .is_some_and(|p| p.disable_self_validation)
        {
            warn!("#\n#\n# WARNING: ConductorConfig.tuning_params.disable_self_validation is set to true. This is dangerous and not recommended outside of testing or debugging.\n#\n#");
        }

        let passphrase = match &builder.passphrase {
            Some(p) => p.clone(),
            None => Arc::new(Mutex::new(sodoken::LockedArray::from(vec![]))),
        };

        let keystore = if let Some(keystore) = builder.keystore.clone() {
            keystore.clone()
        } else {
            pub(crate) fn warn_no_encryption() {
                #[cfg(not(feature = "sqlite-encrypted"))]
                {
                    const MSG: &str = "WARNING: running without local db encryption";
                    eprintln!("{}", MSG);
                    println!("{}", MSG);
                    tracing::warn!("{}", MSG);
                }
            }
            let get_passphrase = || -> ConductorResult<SharedLockedArray> {
                match builder.passphrase.as_ref() {
                    None => Err(
                        one_err::OneErr::new("passphrase required for lair keystore api").into(),
                    ),
                    Some(p) => Ok(p.to_owned()),
                }
            };
            match &builder.config.keystore {
                KeystoreConfig::DangerTestKeystore => {
                    holochain_keystore::spawn_test_keystore().await?
                }
                KeystoreConfig::LairServer { connection_url } => {
                    warn_no_encryption();
                    let passphrase = get_passphrase()?;
                    match spawn_lair_keystore(connection_url.clone(), passphrase).await {
                        Ok(keystore) => keystore,
                        Err(err) => {
                            tracing::error!(?err, "Failed to spawn Lair keystore");
                            return Err(err.into());
                        }
                    }
                }
                KeystoreConfig::LairServerInProc { lair_root } => {
                    warn_no_encryption();

                    let keystore_root_path: KeystorePath = match lair_root {
                        Some(lair_root) => lair_root.clone(),
                        None => builder
                            .config
                            .data_root_path
                            .as_ref()
                            .ok_or(ConductorError::NoDataRootPath)?
                            .clone()
                            .try_into()?,
                    };
                    let keystore_config_path = keystore_root_path
                        .as_ref()
                        .join("lair-keystore-config.yaml");
                    let passphrase = get_passphrase()?;

                    match spawn_lair_keystore_in_proc(&keystore_config_path, passphrase).await {
                        Ok(keystore) => keystore,
                        Err(err) => {
                            tracing::error!(?err, "Failed to spawn Lair keystore in process");
                            return Err(err.into());
                        }
                    }
                }
            }
        };

        info!("Conductor startup: passphrase obtained.");

        let Self {
            ribosome_store,
            config,
            ..
        } = builder;

        let config = Arc::new(config);

        let ribosome_store = RwShare::new(ribosome_store);

        crate::conductor::space::set_danger_print_db_secrets(builder.danger_print_db_secrets);
        let spaces = Spaces::new(config.clone(), passphrase).await?;
        let tag = spaces.get_state().await?.tag().clone();

        let tag_ed: Arc<str> = format!("{}_ed", tag.0).into_boxed_str().into();
        let _ = keystore
            .lair_client()
            .new_seed(tag_ed.clone(), None, false)
            .await;

        let compat = NetworkCompatParams {
            ..Default::default()
        };

        let report = match config.network.report {
            holochain_conductor_api::conductor::ReportConfig::None => {
                holochain_p2p::ReportConfig::None
            }
            holochain_conductor_api::conductor::ReportConfig::JsonLines {
                days_retained,
                fetched_op_interval_s,
            } => holochain_p2p::ReportConfig::JsonLines(holochain_p2p::HcReportConfig {
                path: config.reports_path(),
                days_retained,
                fetched_op_interval_s,
            }),
        };

        let net_spaces1 = spaces.clone();
        let net_spaces2 = spaces.clone();
        let conductor_db = spaces.conductor_db.clone();
        let p2p_config = holochain_p2p::HolochainP2pConfig {
            auth_material: config
                .network
                .base64_auth_material
                .as_ref()
                .map(|m| {
                    use base64::prelude::*;
                    BASE64_STANDARD.decode(m).map_err(ConductorError::other)
                })
                .transpose()?,
            get_db_peer_meta: Arc::new(move |dna_hash| {
                let res = net_spaces1.peer_meta_store_db(&dna_hash);
                Box::pin(async move { res.map_err(holochain_p2p::HolochainP2pError::other) })
            }),
            get_db_op_store: Arc::new(move |dna_hash| {
                let res = net_spaces2.dht_db(&dna_hash);
                Box::pin(async move { res.map_err(holochain_p2p::HolochainP2pError::other) })
            }),
            get_conductor_db: Arc::new(move || {
                let conductor_db = conductor_db.clone();
                Box::pin(async move { conductor_db })
            }),
            target_arc_factor: config.network.target_arc_factor,
            network_config: Some(config.network.to_k2_config()?),
            report,
            compat,
            request_timeout: std::time::Duration::from_secs(config.network.request_timeout_s),
            ..Default::default()
        };

        let holochain_p2p =
            match holochain_p2p::spawn_holochain_p2p(p2p_config, keystore.clone()).await {
                Ok(r) => r,
                Err(err) => {
                    tracing::error!(?err, "Error spawning networking");
                    return Err(err.into());
                }
            };

        info!("Conductor startup: networking started.");

        let (post_commit_sender, post_commit_receiver) =
            tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);

        let (outcome_tx, outcome_rx) = futures::channel::mpsc::channel(8);

        let conductor = Conductor::new(
            config.clone(),
            ribosome_store,
            keystore,
            holochain_p2p.clone(),
            spaces,
            post_commit_sender,
            outcome_tx,
        );

        // Create handle
        let handle: ConductorHandle = Arc::new(conductor);

        holochain_p2p.register_handler(handle.clone()).await?;

        Self::finish(
            handle,
            config,
            post_commit_receiver,
            outcome_rx,
            builder.no_print_setup,
        )
        .await
    }

    pub(crate) async fn spawn_post_commit(
        conductor_handle: ConductorHandle,
        receiver: tokio::sync::mpsc::Receiver<PostCommitArgs>,
        stop: StopReceiver,
        duration_metric: PostCommitDurationMetric,
    ) {
        let receiver_stream = tokio_stream::wrappers::ReceiverStream::new(receiver);
        stop.fuse_with(receiver_stream)
            .for_each_concurrent(POST_COMMIT_CONCURRENT_LIMIT, move |post_commit_args| {
                let start = Instant::now();
                let conductor_handle = conductor_handle.clone();
                let duration_metric = duration_metric.clone();
                async move {
                    let PostCommitArgs {
                        host_access,
                        invocation,
                        cell_id,
                    } = post_commit_args;
                    match conductor_handle.clone().get_ribosome(&cell_id) {
                        Ok(ribosome) => {
                            if let Err(e) = ribosome.run_post_commit(host_access, invocation).await
                            {
                                tracing::error!(?e);
                            }
                        }
                        Err(e) => {
                            tracing::error!(?e);
                        }
                    }

                    duration_metric.record(
                        start.elapsed().as_secs_f64(),
                        &[
                            opentelemetry_api::KeyValue::new(
                                "dna_hash",
                                format!("{:?}", cell_id.dna_hash()),
                            ),
                            opentelemetry_api::KeyValue::new(
                                "agent",
                                format!("{:?}", cell_id.agent_pubkey()),
                            ),
                        ],
                    );
                }
            })
            .await;
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub(crate) async fn finish(
        conductor: ConductorHandle,
        config: Arc<ConductorConfig>,
        post_commit_receiver: tokio::sync::mpsc::Receiver<PostCommitArgs>,
        outcome_receiver: OutcomeReceiver,
        no_print_setup: bool,
    ) -> ConductorResult<ConductorHandle> {
        conductor
            .clone()
            .start_scheduler(SCHEDULER_INTERVAL)
            .await?;

        info!("Conductor startup: scheduler task started.");

        let tm = conductor.task_manager();
        let conductor2 = conductor.clone();
        let post_commit_duration_metric = create_post_commit_duration_metric();
        tm.add_conductor_task_unrecoverable("post_commit_receiver", move |stop| {
            Self::spawn_post_commit(
                conductor2,
                post_commit_receiver,
                stop,
                post_commit_duration_metric,
            )
            .map(Ok)
        });

        let configs = config.admin_interfaces.clone().unwrap_or_default();
        conductor
            .clone()
            .initialize_conductor(outcome_receiver, configs)
            .await?;

        if !no_print_setup {
            conductor.print_setup();
        }

        Ok(conductor)
    }

    /// Pass a test keystore in, to ensure that generated test agents
    /// are actually available for signing (especially for tryorama compat)
    pub fn with_keystore(mut self, keystore: MetaLairClient) -> Self {
        self.keystore = Some(keystore);
        self
    }

    /// Build a Conductor with a test environment
    #[cfg(any(test, feature = "test_utils"))]
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(scope = self.config.network.tracing_scope)))]
    pub async fn test(
        self,
        extra_dna_files: &[(CellId, DnaFile)],
    ) -> ConductorResult<ConductorHandle> {
        if rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .is_err()
        {
            tracing::error!("could not set crypto provider for tls");
        }

        let builder = self;

        let keystore = builder
            .keystore
            .clone()
            .unwrap_or_else(holochain_keystore::test_keystore);

        let config = Arc::new(builder.config);
        let spaces = Spaces::new(
            config.clone(),
            Arc::new(Mutex::new(sodoken::LockedArray::from(
                b"passphrase".to_vec(),
            ))),
        )
        .await?;
        let tag = spaces.get_state().await?.tag().clone();

        let tag_ed: Arc<str> = format!("{}_ed", tag.0).into_boxed_str().into();
        let _ = keystore
            .lair_client()
            .new_seed(tag_ed.clone(), None, false)
            .await;

        let ribosome_store = RwShare::new(builder.ribosome_store);

        let compat = NetworkCompatParams::default();

        let report = match config.network.report {
            holochain_conductor_api::conductor::ReportConfig::None => {
                holochain_p2p::ReportConfig::None
            }
            holochain_conductor_api::conductor::ReportConfig::JsonLines {
                days_retained,
                fetched_op_interval_s,
            } => holochain_p2p::ReportConfig::JsonLines(holochain_p2p::HcReportConfig {
                path: config.reports_path(),
                days_retained,
                fetched_op_interval_s,
            }),
        };

        let net_spaces1 = spaces.clone();
        let net_spaces2 = spaces.clone();
        let conductor_db = spaces.conductor_db.clone();
        let p2p_config = holochain_p2p::HolochainP2pConfig {
            auth_material: config
                .network
                .base64_auth_material
                .as_ref()
                .map(|m| {
                    use base64::prelude::*;
                    BASE64_STANDARD.decode(m).map_err(ConductorError::other)
                })
                .transpose()?,
            get_db_peer_meta: Arc::new(move |dna_hash| {
                let res = net_spaces1.peer_meta_store_db(&dna_hash);
                Box::pin(async move { res.map_err(holochain_p2p::HolochainP2pError::other) })
            }),
            get_db_op_store: Arc::new(move |dna_hash| {
                let res = net_spaces2.dht_db(&dna_hash);
                Box::pin(async move { res.map_err(holochain_p2p::HolochainP2pError::other) })
            }),
            get_conductor_db: Arc::new(move || {
                let conductor_db = conductor_db.clone();
                Box::pin(async move { conductor_db })
            }),
            target_arc_factor: config.network.target_arc_factor,
            network_config: Some(config.network.to_k2_config()?),
            report,
            compat,
            request_timeout: std::time::Duration::from_secs(config.network.request_timeout_s),
            #[cfg(feature = "test_utils")]
            disable_bootstrap: config.network.disable_bootstrap,
            #[cfg(feature = "test_utils")]
            disable_publish: config.network.disable_publish,
            #[cfg(feature = "test_utils")]
            disable_gossip: config.network.disable_gossip,
            ..Default::default()
        };

        let holochain_p2p =
            holochain_p2p::spawn_holochain_p2p(p2p_config, keystore.clone()).await?;

        let (post_commit_sender, post_commit_receiver) =
            tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);

        let (outcome_tx, outcome_rx) = futures::channel::mpsc::channel(8);

        let conductor = Conductor::new(
            config.clone(),
            ribosome_store,
            keystore,
            holochain_p2p.clone(),
            spaces,
            post_commit_sender,
            outcome_tx,
        );

        // Create handle
        let handle: ConductorHandle = Arc::new(conductor);

        holochain_p2p.register_handler(handle.clone()).await?;

        // Register extra DNAs. In particular, the ones with InlineZomes will
        // not be registered in the Wasm DB and cannot be automatically loaded
        // on conductor restart. Hence they need to get passed along here
        // via the extra_dna_files argument (populated from the SweetConductor's
        // DnaFile cache) in order to be added to the RibosomeStore manually.
        for (cell_id, dna_file) in extra_dna_files {
            handle
                .register_dna_file(cell_id.clone(), dna_file.clone())
                .await
                .expect("Could not install DNA");
        }

        Self::finish(
            handle,
            config,
            post_commit_receiver,
            outcome_rx,
            builder.no_print_setup,
        )
        .await
    }
}
