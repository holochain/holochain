use ghost_actor::{GhostControlSender, GhostSender};
use std::{net::SocketAddr, sync::Arc};

use super::{test_keystore, RecordedKitsuneP2pEvent, TestHost, TestHostOp, TestLegacyHost};
use kitsune_p2p::{
    actor::KitsuneP2p, event::KitsuneP2pEventReceiver, spawn_kitsune_p2p, HostApi,
    KitsuneP2pResult, PreflightUserData,
};
use kitsune_p2p_bootstrap::BootstrapShutdown;
use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    config::{tuning_params_struct, KitsuneP2pConfig},
    tls::TlsConfig,
    KAgent,
};
use parking_lot::RwLock;
use tokio::task::AbortHandle;

pub struct KitsuneTestHarness {
    name: String,
    config: KitsuneP2pConfig,
    tls_config: kitsune_p2p_types::tls::TlsConfig,
    host_api: HostApi,
    legacy_host_api: TestLegacyHost,
    agent_store: Arc<RwLock<Vec<AgentInfoSigned>>>,
    op_store: Arc<RwLock<Vec<TestHostOp>>>,
}

impl KitsuneTestHarness {
    pub async fn try_new(name: &str) -> KitsuneP2pResult<Self> {
        let keystore = test_keystore();
        let agent_store = Arc::new(RwLock::new(Vec::new()));
        let op_store = Arc::new(RwLock::new(Vec::new()));

        // Unpack the keystore, since we need to pass it to the host_api
        let keystore = Arc::try_unwrap(keystore).unwrap().into_inner();

        let host_api =
            Arc::new(TestHost::new(keystore.clone(), agent_store.clone(), op_store.clone()).await);
        let legacy_host_api = TestLegacyHost::new(keystore);

        Ok(Self {
            config: KitsuneP2pConfig::empty(),
            name: name.to_string(),
            tls_config: TlsConfig::new_ephemeral().await?,
            host_api,
            legacy_host_api,
            agent_store,
            op_store,
        })
    }

    pub fn configure_tx5_network(mut self, signal_url: SocketAddr) -> Self {
        self.config
            .transport_pool
            .push(kitsune_p2p_types::config::TransportConfig::WebRTC {
                signal_url: format!("ws://{signal_url}"),
                webrtc_config: None,
            });
        self
    }

    pub fn use_bootstrap_server(mut self, bootstrap_addr: SocketAddr) -> Self {
        self.config.bootstrap_service = Some(url2::url2!("http://{:?}", bootstrap_addr));
        self
    }

    pub fn update_tuning_params(
        mut self,
        f: impl Fn(
            tuning_params_struct::KitsuneP2pTuningParams,
        ) -> tuning_params_struct::KitsuneP2pTuningParams,
    ) -> Self {
        let new_config: KitsuneP2pConfig = self.config.tune(f);
        self.config = new_config;
        self
    }

    pub async fn spawn(&mut self) -> KitsuneP2pResult<GhostSender<KitsuneP2p>> {
        let (sender, receiver) = self.spawn_without_legacy_host(self.name.clone()).await?;

        self.start_legacy_host(vec![receiver]).await;

        Ok(sender)
    }

    pub async fn spawn_without_legacy_host(
        &mut self,
        name: String,
    ) -> KitsuneP2pResult<(GhostSender<KitsuneP2p>, KitsuneP2pEventReceiver)> {
        let mut config = self.config.clone();
        config.tracing_scope = Some(name);

        let (sender, receiver) = spawn_kitsune_p2p(
            config,
            self.tls_config.clone(),
            self.host_api.clone(),
            PreflightUserData::default(),
        )
        .await?;

        Ok((sender, receiver))
    }

    pub async fn start_legacy_host(&mut self, receivers: Vec<KitsuneP2pEventReceiver>) {
        self.legacy_host_api
            .start(self.agent_store.clone(), self.op_store.clone(), receivers)
            .await;
    }

    /// Attempts to do a reasonably realistic restart of the Kitsune module.
    /// The host is restarted but not recreated so that in-memory state like the peer store and op data is retained.
    ///
    /// Provide the `sender` that you got from calling `spawn`.
    pub async fn simulated_restart(
        &mut self,
        sender: GhostSender<KitsuneP2p>,
    ) -> KitsuneP2pResult<GhostSender<KitsuneP2p>> {
        // Shutdown the Kitsune module
        sender.ghost_actor_shutdown_immediate().await?;

        // Shutdown the legacy host so that it can be started with a channel to the new Kitsune module
        self.legacy_host_api.shutdown();

        // Start up again
        self.spawn().await
    }

    pub async fn create_agent(&mut self) -> KAgent {
        self.legacy_host_api.create_agent().await
    }

    #[allow(dead_code)]
    pub fn agent_store(&self) -> Arc<parking_lot::RwLock<Vec<AgentInfoSigned>>> {
        self.agent_store.clone()
    }

    pub fn op_store(&self) -> Arc<parking_lot::RwLock<Vec<TestHostOp>>> {
        self.op_store.clone()
    }

    pub async fn drain_legacy_host_events(&mut self) -> Vec<RecordedKitsuneP2pEvent> {
        self.legacy_host_api.drain_events().await
    }

    #[allow(dead_code)]
    pub fn duplicate_ops_received_count(&self) -> u32 {
        self.legacy_host_api.duplicate_ops_received_count()
    }
}

pub struct TestBootstrapHandle {
    shutdown_cb: Option<BootstrapShutdown>,
    abort_handle: AbortHandle,
}

impl TestBootstrapHandle {
    fn new(shutdown_cb: BootstrapShutdown, abort_handle: AbortHandle) -> Self {
        Self {
            shutdown_cb: Some(shutdown_cb),
            abort_handle,
        }
    }

    pub fn abort(&mut self) {
        if let Some(shutdown_cb) = self.shutdown_cb.take() {
            shutdown_cb();
        }
        self.abort_handle.abort();
    }
}

impl Drop for TestBootstrapHandle {
    fn drop(&mut self) {
        self.abort();
    }
}

pub async fn start_bootstrap() -> (SocketAddr, TestBootstrapHandle) {
    let (bs_driver, bs_addr, shutdown) =
        kitsune_p2p_bootstrap::run("127.0.0.1:0".parse::<SocketAddr>().unwrap(), vec![])
            .await
            .expect("Could not start bootstrap server");

    let abort_handle = tokio::spawn(async move {
        bs_driver.await;
    })
    .abort_handle();

    (bs_addr, TestBootstrapHandle::new(shutdown, abort_handle))
}

pub async fn start_signal_srv() -> (SocketAddr, sbd_server::SbdServer) {
    let server = sbd_server::SbdServer::new(Arc::new(sbd_server::Config {
        bind: vec!["127.0.0.1:0".to_string(), "[::1]:0".to_string()],
        limit_clients: 100,
        ..Default::default()
    }))
    .await
    .unwrap();

    (*server.bind_addrs().first().unwrap(), server)
}
