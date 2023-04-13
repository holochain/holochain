use std::sync::Arc;

/// How conductors should learn about each other / speak to each other.
/// Just a bootstrap server in tx2 mode.
/// Signal/TURN + bootstrap in tx5 mode.
pub trait SweetRendezvous: 'static + Send + Sync {
    /// Get the bootstrap address.
    fn bootstrap_addr(&self) -> &str;

    #[cfg(feature = "tx5")]
    /// Get the signal server address.
    fn sig_addr(&self) -> &str;
}

/// Trait object rendezvous.
pub type DynSweetRendezvous = Arc<dyn SweetRendezvous + 'static + Send + Sync>;

/// Local rendezvous infrastructure for unit testing.
pub struct SweetLocalRendezvous {
    bs_addr: String,
    bs_shutdown: Option<kitsune_p2p_bootstrap::BootstrapShutdown>,

    #[cfg(feature = "tx5")]
    turn_srv: Option<tx5_go_pion_turn::Tx5TurnServer>,
    #[cfg(feature = "tx5")]
    sig_addr: String,
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
        let bs_addr = format!("http://{bs_addr}");
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
            let sig_addr: std::net::SocketAddr = (addr, sig_port).into();
            let sig_shutdown = tokio::task::spawn(sig_driver);
            let sig_addr = format!("ws://{sig_addr}");
            tracing::info!("RUNNING SIG: {sig_addr:?}");

            Arc::new(Self {
                bs_addr,
                bs_shutdown: Some(bs_shutdown),
                turn_srv: Some(turn_srv),
                sig_addr,
                sig_shutdown: Some(sig_shutdown),
            })
        }
    }
}

impl SweetRendezvous for SweetLocalRendezvous {
    /// Get the bootstrap address.
    fn bootstrap_addr(&self) -> &str {
        self.bs_addr.as_str()
    }

    #[cfg(feature = "tx5")]
    /// Get the signal server address.
    fn sig_addr(&self) -> &str {
        self.sig_addr.as_str()
    }
}
