use std::sync::Arc;

/// How conductors should learn about each other / speak to each other.
/// Signal/TURN + bootstrap in tx5 mode.
pub trait SweetRendezvous: 'static + Send + Sync {
    /// Get the bootstrap address.
    fn bootstrap_addr(&self) -> &str;

    /// Get the signal server address.
    fn sig_addr(&self) -> &str;
}

/// Trait object rendezvous.
pub type DynSweetRendezvous = Arc<dyn SweetRendezvous + 'static + Send + Sync>;

/// Local rendezvous infrastructure for unit testing.
pub struct SweetLocalRendezvous {
    bs_addr: String,
    bs_shutdown: Option<kitsune_p2p_bootstrap::BootstrapShutdown>,

    turn_srv: Option<tx5_go_pion_turn::Tx5TurnServer>,
    sig_addr: String,
    _sig_hnd: tx5_signal_srv::SrvHnd,
}

impl Drop for SweetLocalRendezvous {
    fn drop(&mut self) {
        if let Some(s) = self.bs_shutdown.take() {
            s();
        }
        if let Some(s) = self.turn_srv.take() {
            tokio::task::spawn(async move {
                let _ = s.stop().await;
            });
        }
    }
}

impl SweetLocalRendezvous {
    /// Create a new local rendezvous instance.
    #[allow(clippy::new_ret_no_self)]
    pub async fn new() -> DynSweetRendezvous {
        let mut addr = None;

        for iface in get_if_addrs::get_if_addrs().expect("failed to get_if_addrs") {
            if iface.ip().is_ipv6() {
                continue;
            }
            addr = Some(iface.ip());
            break;
        }

        let addr = addr.expect("no matching network interfaces found");

        let (bs_driver, bs_addr, bs_shutdown) = kitsune_p2p_bootstrap::run((addr, 0), Vec::new())
            .await
            .unwrap();
        tokio::task::spawn(bs_driver);
        let bs_addr = format!("http://{bs_addr}");
        tracing::info!("RUNNING BOOTSTRAP: {bs_addr:?}");

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

            let (_sig_hnd, sig_addr_list, _sig_err_list) =
                tx5_signal_srv::exec_tx5_signal_srv(sig_conf).await.unwrap();
            let sig_port = sig_addr_list.first().unwrap().port();
            let sig_addr: std::net::SocketAddr = (addr, sig_port).into();
            let sig_addr = format!("ws://{sig_addr}");
            tracing::info!("RUNNING SIG: {sig_addr:?}");

            Arc::new(Self {
                bs_addr,
                bs_shutdown: Some(bs_shutdown),
                turn_srv: Some(turn_srv),
                sig_addr,
                _sig_hnd,
            })
        }
    }
}

impl SweetRendezvous for SweetLocalRendezvous {
    /// Get the bootstrap address.
    fn bootstrap_addr(&self) -> &str {
        self.bs_addr.as_str()
    }

    /// Get the signal server address.
    fn sig_addr(&self) -> &str {
        self.sig_addr.as_str()
    }
}
