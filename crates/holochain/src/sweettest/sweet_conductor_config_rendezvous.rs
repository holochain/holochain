use std::sync::{Arc, Mutex};

/// How conductors should learn about each other / speak to each other.
/// Signal/TURN + bootstrap in tx5 mode.
pub trait SweetRendezvous: 'static + Send + Sync {
    /// Get the bootstrap address.
    fn bootstrap_addr(&self) -> &str;

    /// Get the signal server address.
    fn sig_addr(&self) -> &str;
}

/// Trait object rendezvous.
pub type DynSweetRendezvous = Arc<dyn SweetRendezvous>;

/// Local rendezvous infrastructure for unit testing.
pub struct SweetLocalRendezvous {
    bs_addr: String,
    bs_shutdown: Option<kitsune_p2p_bootstrap::BootstrapShutdown>,

    turn_srv: Option<tx5_go_pion_turn::Tx5TurnServer>,
    sig_addr: String,
    sig_hnd: Mutex<Option<sbd_server::SbdServer>>,
    sig_ip: std::net::IpAddr,
    sig_port: u16,
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
    pub async fn drop_sig(&self) {
        self.sig_hnd.lock().unwrap().take();

        // wait up to 1 second until the socket is actually closed
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;

            match tokio::net::TcpStream::connect((self.sig_ip, self.sig_port)).await {
                Ok(_) => (),
                Err(_) => break,
            }
        }
    }

    /// Start (or restart) the signal server.
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

    /// Get the signal server address.
    fn sig_addr(&self) -> &str {
        self.sig_addr.as_str()
    }
}
