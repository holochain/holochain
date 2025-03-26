use std::net::SocketAddr;
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
    sig_addr: String,
    bootstrap_hnd: Mutex<Option<kitsune2_bootstrap_srv::BootstrapSrv>>,
    bootstrap_addr: SocketAddr,
}

impl Drop for SweetLocalRendezvous {
    fn drop(&mut self) {
        if let Some(mut s) = self.bootstrap_hnd.lock().unwrap().take() {
            if let Err(err) = s.shutdown() {
                tracing::error!(?err, "failed to shutdown bootstrap server");
            }
        }
    }
}

async fn spawn_test_bootstrap(
    bind_addr: Option<SocketAddr>,
) -> std::io::Result<(kitsune2_bootstrap_srv::BootstrapSrv, SocketAddr)> {
    let mut config = kitsune2_bootstrap_srv::Config::testing();
    config.sbd.limit_clients = 100;
    config.sbd.disable_rate_limiting = true;

    if let Some(bind_addr) = bind_addr {
        config.listen_address_list = vec![bind_addr];
    }

    let bootstrap = tokio::task::spawn_blocking(|| {
        tracing::info!("Starting bootstrap server");
        kitsune2_bootstrap_srv::BootstrapSrv::new(config)
    })
    .await
    .unwrap()?;

    tracing::info!("Bootstrap server started");
    let addr = bootstrap.listen_addrs().first().unwrap().clone();

    Ok((bootstrap, addr))
}

impl SweetLocalRendezvous {
    /// Create a new local rendezvous instance.
    #[allow(clippy::new_ret_no_self)]
    pub async fn new() -> DynSweetRendezvous {
        Self::new_raw().await
    }

    /// Create a new local rendezvous instance.
    pub async fn new_raw() -> Arc<Self> {
        let (bootstrap, bootstrap_addr) = spawn_test_bootstrap(None).await.unwrap();

        let bootstrap_hnd = Mutex::new(Some(bootstrap));

        Arc::new(Self {
            bs_addr: format!("http://{bootstrap_addr}"),
            sig_addr: format!("ws://{bootstrap_addr}"),
            bootstrap_hnd,
            bootstrap_addr,
        })
    }

    /// Drop (shutdown) the signal server.
    pub async fn drop_sig(&self) {
        self.bootstrap_hnd.lock().unwrap().take();

        // wait up to 1 second until the socket is actually closed
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;

            match tokio::net::TcpStream::connect(self.bootstrap_addr).await {
                Ok(_) => (),
                Err(_) => break,
            }
        }
    }

    /// Start (or restart) the signal server.
    pub async fn start_sig(&self) {
        self.drop_sig().await;

        let (bootstrap, _) = spawn_test_bootstrap(Some(self.bootstrap_addr))
            .await
            .unwrap();

        *self.bootstrap_hnd.lock().unwrap() = Some(bootstrap);
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
