use std::{net::SocketAddr, sync::Arc};

use super::{test_keystore, RecordedKitsuneP2pEvent, TestHost, TestHostOp, TestLegacyHost};
use kitsune_p2p::{
    actor::KitsuneP2p, event::KitsuneP2pEventReceiver, spawn_kitsune_p2p, HostApi,
    KitsuneP2pResult, PreflightUserData,
};
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
            name: name.to_string(),
            config: Default::default(),
            tls_config: TlsConfig::new_ephemeral().await?,
            host_api,
            legacy_host_api,
            agent_store,
            op_store,
        })
    }

    #[cfg(feature = "tx5")]
    pub fn configure_tx5_network(mut self, signal_url: SocketAddr) -> Self {
        self.config
            .transport_pool
            .push(kitsune_p2p_types::config::TransportConfig::WebRTC {
                signal_url: format!("ws://{signal_url}"),
            });
        self
    }

    pub fn use_bootstrap_server(mut self, bootstrap_addr: SocketAddr) -> Self {
        self.config.network_type = kitsune_p2p_types::config::NetworkType::QuicBootstrap;
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

    pub async fn spawn(&mut self) -> KitsuneP2pResult<ghost_actor::GhostSender<KitsuneP2p>> {
        let (sender, receiver) = self.spawn_without_legacy_host(self.name.clone()).await?;

        self.start_legacy_host(vec![receiver]).await;

        Ok(sender)
    }

    pub async fn spawn_without_legacy_host(
        &mut self,
        name: String,
    ) -> KitsuneP2pResult<(
        ghost_actor::GhostSender<KitsuneP2p>,
        KitsuneP2pEventReceiver,
    )> {
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

pub async fn start_bootstrap() -> (SocketAddr, AbortHandle) {
    let (bs_driver, bs_addr, shutdown) =
        kitsune_p2p_bootstrap::run("127.0.0.1:0".parse::<SocketAddr>().unwrap(), vec![])
            .await
            .expect("Could not start bootstrap server");

    let abort_handle = tokio::spawn(async move {
        let _shutdown_cb = shutdown;
        bs_driver.await;
    })
    .abort_handle();

    (bs_addr, abort_handle)
}

pub async fn start_signal_srv() -> (SocketAddr, tx5_signal_srv::SrvHnd) {
    let mut config = tx5_signal_srv::Config::default();
    config.interfaces = "127.0.0.1".to_string();
    config.port = 0;
    config.demo = false;
    let (sig_hnd, addr_list, _err_list) =
        tx5_signal_srv::exec_tx5_signal_srv(config).await.unwrap();

    (*addr_list.first().unwrap(), sig_hnd)
}
