use holochain_p2p::dht::ArqStrat;

use super::*;
use crate::conductor::kitsune_host_impl::KitsuneHostImpl;
use crate::conductor::ribosome_store::RibosomeStore;
use crate::conductor::ConductorHandle;

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
    #[cfg(any(test, feature = "test_utils"))]
    /// Optional state override (for testing)
    pub state: Option<ConductorState>,
}

impl ConductorBuilder {
    /// Default ConductorBuilder
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

    /// Initialize a "production" Conductor
    pub async fn build(self) -> ConductorResult<ConductorHandle> {
        tracing::info!(?self.config);

        let keystore = if let Some(keystore) = self.keystore {
            keystore
        } else {
            pub(crate) fn warn_no_encryption() {
                #[cfg(not(feature = "db-encryption"))]
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
                KeystoreConfig::DangerTestKeystore => spawn_test_keystore().await?,
                KeystoreConfig::LairServer { connection_url } => {
                    warn_no_encryption();
                    let passphrase = get_passphrase()?;
                    spawn_lair_keystore(connection_url.clone(), passphrase).await?
                }
                KeystoreConfig::LairServerInProc { lair_root } => {
                    warn_no_encryption();
                    let mut keystore_config_path = lair_root.clone().unwrap_or_else(|| {
                        let mut p: std::path::PathBuf = self.config.environment_path.clone().into();
                        p.push("keystore");
                        p
                    });
                    keystore_config_path.push("lair-keystore-config.yaml");
                    let passphrase = get_passphrase()?;
                    spawn_lair_keystore_in_proc(keystore_config_path, passphrase).await?
                }
            }
        };

        let Self {
            ribosome_store,
            config,
            ..
        } = self;

        let ribosome_store = RwShare::new(ribosome_store);

        let spaces = Spaces::new(&config)?;
        let tag = spaces.get_state().await?.tag().clone();

        let network_config = config.network.clone().unwrap_or_default();
        let (cert_digest, cert, cert_priv_key) =
            keystore.get_or_create_tls_cert_by_tag(tag.0).await?;
        let tls_config =
            holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_types::tls::TlsConfig {
                cert,
                cert_priv_key,
                cert_digest,
            };
        let strat = ArqStrat::from_params(network_config.tuning_params.gossip_redundancy_target);

        let host = KitsuneHostImpl::new(
            spaces.clone(),
            ribosome_store.clone(),
            network_config.tuning_params.clone(),
            strat,
        );

        let (holochain_p2p, p2p_evt) =
            holochain_p2p::spawn_holochain_p2p(network_config, tls_config, host).await?;

        let (post_commit_sender, post_commit_receiver) =
            tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);

        let conductor = Conductor::new(
            config.clone(),
            ribosome_store,
            keystore,
            holochain_p2p,
            spaces,
            post_commit_sender,
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

        Self::finish(handle, config, p2p_evt, post_commit_receiver).await
    }

    pub(crate) fn spawn_post_commit(
        conductor_handle: ConductorHandle,
        receiver: tokio::sync::mpsc::Receiver<PostCommitArgs>,
    ) {
        let receiver_stream = tokio_stream::wrappers::ReceiverStream::new(receiver);
        tokio::task::spawn(receiver_stream.for_each_concurrent(
            POST_COMMIT_CONCURRENT_LIMIT,
            move |post_commit_args| {
                let conductor_handle = conductor_handle.clone();
                async move {
                    let PostCommitArgs {
                        host_access,
                        invocation,
                        cell_id,
                    } = post_commit_args;
                    match conductor_handle.clone().get_ribosome(cell_id.dna_hash()) {
                        Ok(ribosome) => {
                            if let Err(e) = tokio::task::spawn_blocking(move || {
                                if let Err(e) = ribosome.run_post_commit(host_access, invocation) {
                                    tracing::error!(?e);
                                }
                            })
                            .await
                            {
                                tracing::error!(?e);
                            }
                        }
                        Err(e) => {
                            tracing::error!(?e);
                        }
                    }
                }
            },
        ));
    }

    pub(crate) async fn finish(
        handle: ConductorHandle,
        conductor_config: ConductorConfig,
        p2p_evt: holochain_p2p::event::HolochainP2pEventReceiver,
        post_commit_receiver: tokio::sync::mpsc::Receiver<PostCommitArgs>,
    ) -> ConductorResult<ConductorHandle> {
        handle
            .clone()
            .start_scheduler(holochain_zome_types::schedule::SCHEDULER_INTERVAL)
            .await;

        tokio::task::spawn(p2p_event_task(p2p_evt, handle.clone()));

        Self::spawn_post_commit(handle.clone(), post_commit_receiver);

        let configs = conductor_config.admin_interfaces.unwrap_or_default();
        let cell_startup_errors = handle.clone().initialize_conductor(configs).await?;

        // TODO: This should probably be emitted over the admin interface
        if !cell_startup_errors.is_empty() {
            error!(
                msg = "Failed to create the following active apps",
                ?cell_startup_errors
            );
        }

        handle.print_setup();

        Ok(handle)
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
    pub async fn test(
        mut self,
        env_path: &std::path::Path,
        extra_dnas: &[DnaFile],
    ) -> ConductorResult<ConductorHandle> {
        let keystore = self.keystore.unwrap_or_else(test_keystore);
        self.config.environment_path = env_path.to_path_buf().into();

        let spaces = Spaces::new(&self.config)?;

        let network_config = self.config.network.clone().unwrap_or_default();
        let tuning_params = network_config.tuning_params.clone();
        let strat = ArqStrat::from_params(tuning_params.gossip_redundancy_target);

        let ribosome_store = RwShare::new(self.ribosome_store);
        let host =
            KitsuneHostImpl::new(spaces.clone(), ribosome_store.clone(), tuning_params, strat);

        let (holochain_p2p, p2p_evt) =
                holochain_p2p::spawn_holochain_p2p(network_config, holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_types::tls::TlsConfig::new_ephemeral().await.unwrap(), host)
                    .await?;

        let (post_commit_sender, post_commit_receiver) =
            tokio::sync::mpsc::channel(POST_COMMIT_CHANNEL_BOUND);

        let conductor = Conductor::new(
            self.config.clone(),
            ribosome_store,
            keystore,
            holochain_p2p,
            spaces,
            post_commit_sender,
        );

        let conductor = Self::update_fake_state(self.state, conductor).await?;

        // Create handle
        let handle: ConductorHandle = Arc::new(conductor);

        // Install extra DNAs, in particular:
        // the ones with InlineZomes will not be registered in the Wasm DB
        // and cannot be automatically loaded on conductor restart.

        for dna_file in extra_dnas {
            handle
                .register_dna(dna_file.clone())
                .await
                .expect("Could not install DNA");
        }

        Self::finish(handle, self.config, p2p_evt, post_commit_receiver).await
    }
}
