use super::*;
use crate::conductor::kitsune_host_impl::KitsuneHostImpl;
use crate::conductor::manager::OutcomeReceiver;
use crate::conductor::metrics::{create_post_commit_duration_metric, PostCommitDurationMetric};
use crate::conductor::paths::DataRootPath;
use crate::conductor::ribosome_store::RibosomeStore;
use crate::conductor::ConductorHandle;
use holochain_conductor_api::conductor::paths::KeystorePath;
use holochain_p2p::NetworkCompatParams;

/// A configurable Builder for Conductor and sometimes ConductorHandle
#[derive(Default)]
pub struct ConductorBuilder {
    /// The configuration
    pub config: ConductorConfig,

    /// The RibosomeStore (mockable)
    pub ribosome_store: RibosomeStore,

    /// For new lair, passphrase is required
    pub passphrase: Option<sodoken::BufRead>,

    /// Optional keystore override
    pub keystore: Option<MetaLairClient>,

    /// Optional state override (for testing)
    #[cfg(any(test, feature = "test_utils"))]
    pub state: Option<ConductorState>,

    /// Optional DPKI service implementation, used to override the service specified in the config,
    /// especially for testing with a mock
    #[cfg(any(test, feature = "test_utils"))]
    pub dpki: Option<DpkiImpl>,

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
    pub fn passphrase(mut self, passphrase: Option<sodoken::BufRead>) -> Self {
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
        tracing::debug!(?self.config);

        let passphrase = match &self.passphrase {
            Some(p) => p.clone(),
            None => sodoken::BufRead::new_no_lock(&[]),
        };

        let keystore = if let Some(keystore) = self.keystore {
            keystore
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
            let get_passphrase = || -> ConductorResult<sodoken::BufRead> {
                match self.passphrase {
                    None => Err(
                        one_err::OneErr::new("passphrase required for lair keystore api").into(),
                    ),
                    Some(p) => Ok(p),
                }
            };
            match &self.config.keystore {
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
                        None => self
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
        } = self;

        let config = Arc::new(config);

        let ribosome_store = RwShare::new(ribosome_store);

        crate::conductor::space::set_danger_print_db_secrets(self.danger_print_db_secrets);
        let spaces = Spaces::new(config.clone(), passphrase).await?;
        let tag = spaces.get_state().await?.tag().clone();

        let tag_ed: Arc<str> = format!("{}_ed", tag.0).into_boxed_str().into();
        let _ = keystore
            .lair_client()
            .new_seed(tag_ed.clone(), None, false)
            .await;

        let network_config = config.network.clone();
        let (cert_digest, cert, cert_priv_key) = keystore
            .get_or_create_tls_cert_by_tag(tag.0.clone())
            .await?;
        let tls_config =
            holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_types::tls::TlsConfig {
                cert,
                cert_priv_key,
                cert_digest,
            };

        info!("Conductor startup: TLS cert created.");

        let strat = network_config.tuning_params.to_arq_strat();

        let host = KitsuneHostImpl::new(
            spaces.clone(),
            config.clone(),
            ribosome_store.clone(),
            strat,
            Some(tag_ed),
            Some(keystore.lair_client()),
        );

        // TODO: when we make DPKI optional, we can remove the unwrap_or and just let it be None,
        let dpki_config = Some(config.dpki.clone());

        let dpki_dna_to_install = match &dpki_config {
            Some(config) => {
                if config.no_dpki {
                    None
                } else {
                    let dna = get_dpki_dna(config)
                        .await?
                        .into_dna_file(Default::default())
                        .await?
                        .0;

                    Some(dna)
                }
            }
            _ => unreachable!(
                "We currently require DPKI to be used, but this may change in the future"
            ),
        };

        let dpki_uuid = dpki_dna_to_install
            .as_ref()
            .map(|dna| dna.dna_hash().get_raw_32().try_into().expect("32 bytes"));
        let network_compat = NetworkCompatParams { dpki_uuid };

        let (holochain_p2p, p2p_evt) = match holochain_p2p::spawn_holochain_p2p(
            network_config,
            tls_config,
            host,
            network_compat,
        )
        .await
        {
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
            holochain_p2p,
            spaces,
            post_commit_sender,
            outcome_tx,
        );

        let shutting_down = conductor.shutting_down.clone();

        #[cfg(any(test, feature = "test_utils"))]
        let conductor = Self::update_fake_state(self.state, conductor).await?;

        // Create handle
        let handle: ConductorHandle = Arc::new(conductor);

        {
            let handle = handle.clone();
            tokio::task::spawn(async move {
                while !shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    if let Err(e) = handle.prune_p2p_agents_db().await {
                        tracing::error!("failed to prune p2p_agents_db: {:?}", e);
                    }
                }
            });
        }

        Self::finish(
            handle,
            config,
            dpki_dna_to_install,
            p2p_evt,
            post_commit_receiver,
            outcome_rx,
            self.no_print_setup,
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
                    match conductor_handle.clone().get_ribosome(cell_id.dna_hash()) {
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
        dpki_dna_to_install: Option<DnaFile>,
        p2p_evt: holochain_p2p::event::HolochainP2pEventReceiver,
        post_commit_receiver: tokio::sync::mpsc::Receiver<PostCommitArgs>,
        outcome_receiver: OutcomeReceiver,
        no_print_setup: bool,
    ) -> ConductorResult<ConductorHandle> {
        conductor
            .clone()
            .start_scheduler(SCHEDULER_INTERVAL)
            .await?;

        info!("Conductor startup: scheduler task started.");

        tokio::task::spawn(p2p_event_task(p2p_evt, conductor.clone()).in_current_span());

        info!("Conductor startup: p2p event task started.");

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
        let cell_startup_errors = conductor
            .clone()
            .initialize_conductor(outcome_receiver, configs)
            .await?;

        // TODO: This should probably be emitted over the admin interface
        if !cell_startup_errors.is_empty() {
            error!(
                msg = "Failed to create the following active apps",
                ?cell_startup_errors
            );
        }

        // Install DPKI from DNA
        if let Some(dna) = dpki_dna_to_install {
            let dna_hash = dna.dna_hash().clone();
            match conductor.clone().install_dpki(dna, true).await {
                Ok(_) => tracing::info!("Installed DPKI from DNA {}", dna_hash),
                Err(ConductorError::AppAlreadyInstalled(_)) => {
                    tracing::debug!("DPKI already installed, skipping installation")
                }
                Err(e) => return Err(e),
            }
        }

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

    #[cfg(any(test, feature = "test_utils"))]
    /// Sets some fake conductor state for tests
    pub fn fake_state(mut self, state: ConductorState) -> Self {
        self.state = Some(state);
        self
    }

    #[cfg(any(test, feature = "test_utils"))]
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub(crate) async fn update_fake_state(
        state: Option<ConductorState>,
        conductor: Conductor,
    ) -> ConductorResult<Conductor> {
        if let Some(state) = state {
            conductor.update_state(move |_| Ok(state)).await?;
        }
        Ok(conductor)
    }

    /// Build a Conductor with a test environment
    #[cfg(any(test, feature = "test_utils"))]
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(scope = self.config.network.tracing_scope)))]
    pub async fn test(self, extra_dnas: &[DnaFile]) -> ConductorResult<ConductorHandle> {
        let keystore = self
            .keystore
            .unwrap_or_else(holochain_keystore::test_keystore);

        let config = Arc::new(self.config);
        let spaces =
            Spaces::new(config.clone(), sodoken::BufRead::new_no_lock(b"passphrase")).await?;
        let tag = spaces.get_state().await?.tag().clone();

        let tag_ed: Arc<str> = format!("{}_ed", tag.0).into_boxed_str().into();
        let _ = keystore
            .lair_client()
            .new_seed(tag_ed.clone(), None, false)
            .await;

        let network_config = config.network.clone();
        let strat = network_config.tuning_params.to_arq_strat();

        let ribosome_store = RwShare::new(self.ribosome_store);
        let host = KitsuneHostImpl::new(
            spaces.clone(),
            config.clone(),
            ribosome_store.clone(),
            strat,
            Some(tag_ed),
            Some(keystore.lair_client()),
        );

        // TODO: when we make DPKI optional, we can remove the unwrap_or and just let it be None,
        let dpki_config = Some(config.dpki.clone());

        let (dpki_uuid, dpki_dna_to_install) = match (&self.dpki, &dpki_config) {
            // If a DPKI impl was provided to the builder, use that
            (Some(dpki_impl), _) => (Some(dpki_impl.uuid()), None),

            // Otherwise load the DNA from config if specified
            (None, Some(dpki_config)) => {
                if dpki_config.no_dpki {
                    (None, None)
                } else {
                    let dna = get_dpki_dna(dpki_config)
                        .await?
                        .into_dna_file(Default::default())
                        .await?
                        .0;
                    (
                        Some(dna.dna_hash().get_raw_32().try_into().expect("32 bytes")),
                        Some(dna),
                    )
                }
            }

            (None, None) => unreachable!(
                "We currently require DPKI to be used, but this may change in the future"
            ),
        };

        let network_compat = NetworkCompatParams { dpki_uuid };

        let (holochain_p2p, p2p_evt) =
                holochain_p2p::spawn_holochain_p2p(network_config, holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_types::tls::TlsConfig::new_ephemeral().await.unwrap(), host, network_compat)
                    .await?;

        let (post_commit_sender, post_commit_receiver) =
            tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);

        let (outcome_tx, outcome_rx) = futures::channel::mpsc::channel(8);

        let conductor = Conductor::new(
            config.clone(),
            ribosome_store,
            keystore,
            holochain_p2p,
            spaces,
            post_commit_sender,
            outcome_tx,
        );

        let conductor = Self::update_fake_state(self.state, conductor).await?;

        // Create handle
        let handle: ConductorHandle = Arc::new(conductor);

        // Install DPKI from DNA or mock
        if let Some(dpki_impl) = self.dpki {
            // This is a mock DPKI impl, so inject it into the conductor directly
            handle.running_services_mutex().share_mut(|s| {
                s.dpki = Some(dpki_impl);
            });
        }

        // Install extra DNAs, in particular:
        // the ones with InlineZomes will not be registered in the Wasm DB
        // and cannot be automatically loaded on conductor restart.

        for dna_file in extra_dnas {
            handle
                .register_dna(dna_file.clone())
                .await
                .expect("Could not install DNA");
        }

        Self::finish(
            handle,
            config,
            dpki_dna_to_install,
            p2p_evt,
            post_commit_receiver,
            outcome_rx,
            self.no_print_setup,
        )
        .await
    }
}
