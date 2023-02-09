use std::sync::Arc;

use holochain_conductor_api::{conductor::ConductorConfig, AdminInterfaceConfig, InterfaceDriver};
use kitsune_p2p::KitsuneP2pConfig;

/// Local rendezvous infrastructure for unit testing.
/// Just a bootstrap server in tx2 mode.
/// Signal/TURN + bootstrap in tx5 mode.
pub struct LocalRendezvous {
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

impl Drop for LocalRendezvous {
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

impl LocalRendezvous {
    /// Create a new local rendezvous instance.
    pub async fn new() -> Self {
        let (bs_driver, bs_addr, bs_shutdown) = kitsune_p2p_bootstrap::run(([0, 0, 0, 0], 0), Vec::new()).await.unwrap();
        tokio::task::spawn(bs_driver);
        tracing::info!("RUNNING BOOTSTRAP: {bs_addr:?}");

        #[cfg(not(feature = "tx5"))]
        {
            Self {
                bs_addr,
                bs_shutdown: Some(bs_shutdown),
            }
        }

        #[cfg(feature = "tx5")]
        {
            let (turn_addr, turn_srv) = tx5_go_pion_turn::test_turn_server().await.unwrap();
            tracing::info!("RUNNING TURN: {turn_addr:?}");

            let mut sig_conf = tx5_signal_srv::Config::default();
            sig_conf.port = 0;
            sig_conf.ice_servers = serde_json::from_str(&turn_addr).unwrap();
            sig_conf.demo = false;

            let (sig_addr, sig_driver) = tx5_signal_srv::exec_tx5_signal_srv(sig_conf).unwrap();
            let sig_shutdown = tokio::task::spawn(sig_driver);
            tracing::info!("RUNNING SIG: {sig_addr:?}");

            Self {
                bs_addr,
                bs_shutdown: Some(bs_shutdown),
                turn_addr,
                turn_srv: Some(turn_srv),
                sig_addr,
                sig_shutdown: Some(sig_shutdown),
            }
        }
    }

    /// Get the bootstrap address.
    pub fn bootstrap_addr(&self) -> std::net::SocketAddr {
        self.bs_addr
    }

    #[cfg(feature = "tx5")]
    /// Get the turn server address.
    pub fn turn_addr(&self) -> &str {
        &self.turn_addr
    }

    #[cfg(feature = "tx5")]
    /// Get the signal server address.
    pub fn sig_addr(&self) -> std::net::SocketAddr {
        self.sig_addr
    }
}

/// Wrapper around ConductorConfig with some helpful builder methods
#[derive(
    Clone,
    Debug,
    PartialEq,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
    derive_more::Into,
)]
pub struct SweetConductorConfig(ConductorConfig);

impl SweetConductorConfig {
    /// Standard config for SweetConductors
    pub fn standard() -> Self {
        let mut tuning =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tuning.gossip_strategy = "sharded-gossip".to_string();

        let mut network = KitsuneP2pConfig::default();
        network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
            bind_to: None,
            override_host: None,
            override_port: None,
        }];
        network.tuning_params = Arc::new(tuning);
        Self(ConductorConfig {
            network: Some(network),
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        })
    }

    #[cfg(features = "tx5")]
    /// Setup for webrtc networking
    pub fn webrtc_networking(mut self, signal_url: String) -> Self {
        let mut network = KitsuneP2pConfig::default();
        network.transport_pool = vec![kitsune_p2p::TransportConfig::WebRTC {
            signal_url,
        }];
        self.network = Some(network);
        self
    }

    /// Completely disable networking
    pub fn no_networking(mut self) -> Self {
        if let Some(c) = self.network.as_mut() {
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
        if let Some(c) = self.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp
            });
        }
        self
    }

    /// Disable publishing and recent gossip
    pub fn historical_only(mut self) -> Self {
        if let Some(c) = self.network.as_mut() {
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
        if let Some(c) = self.network.as_mut() {
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
        if let Some(c) = self.network.as_mut() {
            *c = c.clone().tune(|mut tp| {
                tp.disable_publish = true;
                tp.disable_historical_gossip = true;
                tp
            });
        }
        self
    }
}
