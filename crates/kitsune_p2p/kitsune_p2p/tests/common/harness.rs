use std::net::SocketAddr;

use kitsune_p2p::{
    actor::KitsuneP2p, event::KitsuneP2pEventReceiver, spawn_kitsune_p2p, HostApi, KitsuneP2pResult,
};
use kitsune_p2p_types::{config::KitsuneP2pConfig, tls::TlsConfig};
use tokio::task::AbortHandle;

pub struct KitsuneTestHarness {
    config: KitsuneP2pConfig,
    tls_config: kitsune_p2p_types::tls::TlsConfig,
    host_api: HostApi,
}

impl KitsuneTestHarness {
    pub async fn try_new(host_api: HostApi) -> KitsuneP2pResult<Self> {
        Ok(Self {
            config: KitsuneP2pConfig::empty(),
            tls_config: TlsConfig::new_ephemeral().await?,
            host_api,
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
        self.config.bootstrap_service = Some(url2::url2!("http://{:?}", bootstrap_addr));
        self
    }

    pub async fn spawn(
        &mut self,
    ) -> KitsuneP2pResult<(
        ghost_actor::GhostSender<KitsuneP2p>,
        KitsuneP2pEventReceiver,
    )> {
        spawn_kitsune_p2p(
            self.config.clone(),
            self.tls_config.clone(),
            self.host_api.clone(),
        )
        .await
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

pub async fn start_signal_srv() -> (SocketAddr, AbortHandle) {
    let mut config = tx5_signal_srv::Config::default();
    config.interfaces = "127.0.0.1".to_string();
    config.port = 0;
    config.demo = false;
    let (sig_driver, addr_list, _err_list) = tx5_signal_srv::exec_tx5_signal_srv(config).unwrap();

    let abort_handle = tokio::spawn(sig_driver).abort_handle();

    (addr_list.first().unwrap().clone(), abort_handle)
}
