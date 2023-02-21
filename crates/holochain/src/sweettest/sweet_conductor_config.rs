use std::sync::Arc;

use holochain_conductor_api::{conductor::ConductorConfig, AdminInterfaceConfig, InterfaceDriver};
use kitsune_p2p::KitsuneP2pConfig;

tokio::task_local! {
    static RENDEZVOUS: DynSweetRendezvous;
}

/// How conductors should learn about each other / speak to each other.
/// Just a bootstrap server in tx2 mode.
/// Signal/TURN + bootstrap in tx5 mode.
pub trait SweetRendezvous: 'static + Send + Sync {
    /// Get the bootstrap address.
    fn bootstrap_addr(&self) -> std::net::SocketAddr;

    #[cfg(feature = "tx5")]
    /// Get the turn server address.
    fn turn_addr(&self) -> &str;

    #[cfg(feature = "tx5")]
    /// Get the signal server address.
    fn sig_addr(&self) -> std::net::SocketAddr;
}

/// Trait object rendezvous.
pub type DynSweetRendezvous = Arc<dyn SweetRendezvous + 'static + Send + Sync>;

/// Local rendezvous infrastructure for unit testing.
pub struct SweetLocalRendezvous {
    bs_addr: std::net::SocketAddr,
    bs_shutdown: Option<kitsune_p2p_bootstrap::BootstrapShutdown>,

    #[cfg(feature = "tx5")]
    turn_addr: String,
    #[cfg(feature = "tx5")]
    turn_srv: Option<tx5_go_pion_turn::Tx5TurnServer>,
    #[cfg(feature = "tx5")]
    sig_addr: std::net::SocketAddr,
    #[cfg(feature = "tx5")]
    sig_shutdown: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for SweetLocalRendezvous {
    fn drop(&mut self) {
        if let Some(s) = self.bs_shutdown.take() {
            s();
        }
        #[cfg(feature = "tx5")]
        if let Some(s) = self.turn_srv.take() {
            tokio::task::spawn(async move {
                let _ = s.stop().await;
            });
        }
        #[cfg(feature = "tx5")]
        if let Some(s) = self.sig_shutdown.take() {
            s.abort();
        }
    }
}

impl SweetLocalRendezvous {
    /// Create a new local rendezvous instance.
    #[allow(clippy::new_ret_no_self)]
    pub async fn new() -> DynSweetRendezvous {
        let mut addr = None;

        for iface in get_if_addrs::get_if_addrs().expect("failed to get_if_addrs") {
            if iface.is_loopback() {
                continue;
            }
            if iface.ip().is_ipv6() {
                continue;
            }
            addr = Some(iface.ip());
            break;
        }

        let addr = addr.expect("failed to get_if_addrs");

        let (bs_driver, bs_addr, bs_shutdown) = kitsune_p2p_bootstrap::run((addr, 0), Vec::new())
            .await
            .unwrap();
        tokio::task::spawn(bs_driver);
        tracing::info!("RUNNING BOOTSTRAP: {bs_addr:?}");

        #[cfg(not(feature = "tx5"))]
        {
            Arc::new(Self {
                bs_addr,
                bs_shutdown: Some(bs_shutdown),
            })
        }

        #[cfg(feature = "tx5")]
        {
            let (turn_addr, turn_srv) = tx5_go_pion_turn::test_turn_server().await.unwrap();
            tracing::info!("RUNNING TURN: {turn_addr:?}");

            let mut sig_conf = tx5_signal_srv::Config::default();
            sig_conf.port = 0;
            sig_conf.ice_servers = serde_json::json!({
                "iceServers": [
                    serde_json::from_str::<serde_json::Value>(&turn_addr).unwrap(),
                ],
            });
            sig_conf.demo = false;
            tracing::info!(
                "RUNNING ICE SERVERS: {}",
                serde_json::to_string_pretty(&sig_conf.ice_servers).unwrap()
            );

            let (sig_addr, sig_driver) = tx5_signal_srv::exec_tx5_signal_srv(sig_conf).unwrap();
            let sig_port = sig_addr.port();
            let sig_addr = (addr, sig_port).into();
            let sig_shutdown = tokio::task::spawn(sig_driver);
            tracing::info!("RUNNING SIG: {sig_addr:?}");

            Arc::new(Self {
                bs_addr,
                bs_shutdown: Some(bs_shutdown),
                turn_addr,
                turn_srv: Some(turn_srv),
                sig_addr,
                sig_shutdown: Some(sig_shutdown),
            })
        }
    }
}

impl SweetRendezvous for SweetLocalRendezvous {
    /// Get the bootstrap address.
    fn bootstrap_addr(&self) -> std::net::SocketAddr {
        self.bs_addr
    }

    #[cfg(feature = "tx5")]
    /// Get the turn server address.
    fn turn_addr(&self) -> &str {
        &self.turn_addr
    }

    #[cfg(feature = "tx5")]
    /// Get the signal server address.
    fn sig_addr(&self) -> std::net::SocketAddr {
        self.sig_addr
    }
}

/// Wrapper around ConductorConfig with some helpful builder methods
#[derive(Clone, Debug, PartialEq)]
pub struct SweetConductorConfig(ConductorConfig);

impl From<ConductorConfig> for SweetConductorConfig {
    fn from(c: ConductorConfig) -> Self {
        Self(c)
    }
}

impl From<KitsuneP2pConfig> for SweetConductorConfig {
    fn from(network: KitsuneP2pConfig) -> Self {
        ConductorConfig {
            network: Some(network),
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        }
        .into()
    }
}

impl SweetConductorConfig {
    /// Convert into a ConductorConfig.
    pub async fn into_conductor_config(self, rendezvous: &dyn SweetRendezvous) -> ConductorConfig {
        let mut config = self.0;

        if let Some(n) = config.network.as_mut() {
            if n.bootstrap_service.is_some()
                && n.bootstrap_service.as_ref().unwrap().to_string() == "rendezvous:"
            {
                n.bootstrap_service = Some(url2::url2!("http://{}", rendezvous.bootstrap_addr()));
            }

            #[cfg(feature = "tx5")]
            {
                for t in n.transport_pool.iter_mut() {
                    if let kitsune_p2p::TransportConfig::WebRTC { signal_url } = t {
                        if signal_url == "rendezvous:" {
                            *signal_url = format!("ws://{}", rendezvous.sig_addr());
                        }
                    }
                }
            }
        }

        tracing::info!(?config);

        config
    }

    /// Standard config for SweetConductors
    pub fn standard() -> Self {
        let mut tuning =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tuning.gossip_strategy = "sharded-gossip".to_string();

        let mut network = KitsuneP2pConfig::default();
        network.bootstrap_service = Some(url2::url2!("rendezvous:"));

        #[cfg(not(feature = "tx5"))]
        {
            network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
                bind_to: None,
                override_host: None,
                override_port: None,
            }];
        }

        #[cfg(feature = "tx5")]
        {
            network.transport_pool = vec![kitsune_p2p::TransportConfig::WebRTC {
                signal_url: "rendezvous:".into(),
            }];
        }

        network.tuning_params = Arc::new(tuning);
        network.into()
    }

    /// Set network tuning params.
    pub fn tune(
        mut self,
        tuning_params: kitsune_p2p_types::config::KitsuneP2pTuningParams,
    ) -> Self {
        self.0
            .network
            .as_mut()
            .expect("failed to tune network")
            .tuning_params = tuning_params;
        self
    }

    #[cfg(feature = "tx5")]
    /// Setup for webrtc networking
    pub fn webrtc_networking(mut self, signal_url: String) -> Self {
        let mut network = KitsuneP2pConfig::default();
        network.transport_pool = vec![kitsune_p2p::TransportConfig::WebRTC { signal_url }];
        self.0.network = Some(network);
        self
    }

    /// Completely disable networking
    pub fn no_networking(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp.disable_recent_gossip = true;
                tp.disable_historical_gossip = true;
                tp
            });
        }
        self
    }

    /// Disable publishing
    pub fn no_publish(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp
            });
        }
        self
    }

    /// Disable publishing and recent gossip
    pub fn historical_only(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp.disable_recent_gossip = true;
                tp
            });
        }
        self
    }

    /// Disable recent op gossip, but keep agent gossip
    pub fn historical_and_agent_gossip_only(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                // keep recent gossip for agent gossip, but gossip no ops.
                tp.danger_gossip_recent_threshold_secs = 0;
                tp
            });
        }
        self
    }

    /// Disable publishing and historical gossip
    pub fn recent_only(mut self) -> Self {
        if let Some(c) = self.0.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp.disable_historical_gossip = true;
                tp
            });
        }
        self
    }
}
