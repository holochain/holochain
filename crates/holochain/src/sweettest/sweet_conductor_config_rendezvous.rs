use std::sync::{Arc, Mutex};

/// How conductors should learn about each other / speak to each other.
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
    sig_hnd: Mutex<Option<sbd_server::SbdServer>>,
    #[cfg(feature = "tx5")]
    sig_ip: std::net::IpAddr,
    #[cfg(feature = "tx5")]
    sig_port: u16,
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
    }
}

#[cfg(feature = "tx5")]
async fn spawn_sig(ip: std::net::IpAddr, port: u16) -> (String, u16, sbd_server::SbdServer) {
    let sig_hnd = sbd_server::SbdServer::new(Arc::new(sbd_server::Config {
        bind: vec![format!("{ip}:{port}")],
        limit_clients: 100,
        disable_rate_limiting: true,
        ..Default::default()
    }))
    .await
    .unwrap();

    let sig_addr = *sig_hnd.bind_addrs().first().unwrap();
    let sig_port = sig_addr.port();
    let sig_addr = format!("ws://{sig_addr}");
    tracing::info!("RUNNING SIG: {sig_addr:?}");

    (sig_addr, sig_port, sig_hnd)
}

impl SweetLocalRendezvous {
    /// Create a new local rendezvous instance.
    #[allow(clippy::new_ret_no_self)]
    pub async fn new() -> DynSweetRendezvous {
        Self::new_raw().await
    }

    /// Create a new local rendezvous instance.
    pub async fn new_raw() -> Arc<Self> {
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

            let (sig_addr, sig_port, sig_hnd) = spawn_sig(addr, 0).await;

            let sig_hnd = Mutex::new(Some(sig_hnd));

            Arc::new(Self {
                bs_addr,
                bs_shutdown: Some(bs_shutdown),
                turn_srv: Some(turn_srv),
                sig_addr,
                sig_hnd,
                sig_ip: addr,
                sig_port,
            })
        }
    }

    /// Drop (shutdown) the signal server.
    #[cfg(feature = "tx5")]
    pub async fn drop_sig(&self) {
        self.sig_hnd.lock().unwrap().take();

        // NOTE: on windows (and slow other systems) we need to wait a moment
        //       to make sure that the old connection is actually closed.
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }

    /// Start (or restart) the signal server.
    #[cfg(feature = "tx5")]
    pub async fn start_sig(&self) {
        self.drop_sig().await;

        let (_, _, sig_hnd) = spawn_sig(self.sig_ip, self.sig_port).await;

        *self.sig_hnd.lock().unwrap() = Some(sig_hnd);
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
