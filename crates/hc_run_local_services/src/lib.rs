use clap::Parser;
use std::io::{Error, Result};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

/// Helper for running local Holochain bootstrap and WebRTC signal servers.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct HcRunLocalServices {
    /// If set, write the bound address list to a new file, separated by
    /// newlines. If the file exists, an error will be returned.
    #[arg(long)]
    bootstrap_address_path: Option<std::path::PathBuf>,

    /// A single interface on which to run the bootstrap server.
    #[arg(long, default_value = "127.0.0.1")]
    bootstrap_interface: String,

    /// The port to use for the bootstrap server. You probably want
    /// to leave this as 0 (zero) to be assigned an available port.
    #[arg(short, long, default_value = "0")]
    bootstrap_port: u16,

    /// Disable running a bootstrap server.
    #[arg(long)]
    disable_bootstrap: bool,

    /// If set, write the bound address list to a new file, separated by
    /// newlines. If the file exists, an error will be returned.
    #[arg(long)]
    signal_address_path: Option<std::path::PathBuf>,

    /// A comma-separated list of interfaces on which to run the signal server.
    #[arg(long, default_value = "127.0.0.1, [::1]")]
    signal_interfaces: String,

    /// The port to use for the signal server. You probably want
    /// to leave this as 0 (zero) to be assigned an available port.
    #[arg(short, long, default_value = "0")]
    signal_port: u16,

    /// Disable running a signal server.
    #[arg(long)]
    disable_signal: bool,
}

struct AOut(Option<tokio::fs::File>);

impl AOut {
    pub async fn new(p: &Option<std::path::PathBuf>) -> Result<Self> {
        Ok(Self(if let Some(path) = p {
            Some(
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .await?,
            )
        } else {
            None
        }))
    }

    pub async fn write(&mut self, s: String) -> Result<()> {
        if let Some(f) = &mut self.0 {
            f.write_all(s.as_bytes()).await?;
        }
        Ok(())
    }

    pub async fn close(mut self) -> Result<()> {
        if let Some(f) = &mut self.0 {
            f.flush().await?;
            f.shutdown().await?;
        }
        Ok(())
    }
}

impl HcRunLocalServices {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bootstrap_address_path: Option<std::path::PathBuf>,
        bootstrap_interface: String,
        bootstrap_port: u16,
        disable_bootstrap: bool,
        signal_address_path: Option<std::path::PathBuf>,
        signal_interfaces: String,
        signal_port: u16,
        disable_signal: bool,
    ) -> Self {
        Self {
            bootstrap_address_path,
            bootstrap_interface,
            bootstrap_port,
            disable_bootstrap,
            signal_address_path,
            signal_interfaces,
            signal_port,
            disable_signal,
        }
    }

    pub async fn run(self) {
        if let Err(err) = self.run_err().await {
            eprintln!("run-local-services error");
            eprintln!("{err:#?}");
        }
    }

    pub async fn run_err(self) -> Result<()> {
        if self.disable_bootstrap && self.disable_signal {
            tracing::error!("All Services Disabled - Aborting");
            return Ok(());
        }

        let (bootstrap_shutdown_tx, bootstrap_shutdown_rx) = tokio::sync::oneshot::channel();
        if !self.disable_bootstrap {
            let bs_ip: std::net::IpAddr = self.bootstrap_interface.parse().map_err(Error::other)?;
            let bs_addr = std::net::SocketAddr::from((bs_ip, self.bootstrap_port));

            let mut config = kitsune2_bootstrap_srv::Config::testing();
            config.listen_address_list = vec![bs_addr];

            std::thread::Builder::new()
                .name("bootstrap_srv".to_string())
                .spawn(move || {
                    // Signal that the bootstrap server is shutting down if the thread dies.
                    struct D(Option<tokio::sync::oneshot::Sender<()>>);
                    impl Drop for D {
                        fn drop(&mut self) {
                            if let Some(s) = self.0.take() {
                                let _ = s.send(());
                            }
                        }
                    }
                    let _d = D(Some(bootstrap_shutdown_tx));

                    let bootstrap_srv = kitsune2_bootstrap_srv::BootstrapSrv::new(config).unwrap();

                    let rt = tokio::runtime::Builder::new_multi_thread()
                        .enable_io()
                        .build()
                        .unwrap();
                    rt.block_on(async move {
                        let mut a_out = AOut::new(&self.bootstrap_address_path).await?;

                        for addr in tx_addr(bootstrap_srv.listen_addrs()[0])? {
                            a_out.write(format!("http://{addr}\n")).await?;
                            println!("# HC BOOTSTRAP - ADDR: http://{addr}");
                        }

                        a_out.close().await?;

                        println!("# HC BOOTSTRAP - RUNNING");

                        tokio::signal::ctrl_c().await?;

                        drop(bootstrap_srv);

                        std::io::Result::Ok(())
                    })
                    .unwrap();
                })?;
        };

        let sig_hnd = if !self.disable_signal {
            let bind = self
                .signal_interfaces
                .split(',')
                .map(|i| format!("{}:{}", i.trim(), self.signal_port))
                .collect();
            println!("BIND: {bind:?}");
            let config = sbd_server::Config {
                bind,
                ..Default::default()
            };
            tracing::info!(?config);

            let sig_hnd = sbd_server::SbdServer::new(Arc::new(config)).await?;

            let addr_list = sig_hnd.bind_addrs().to_vec();

            let mut a_out = AOut::new(&self.signal_address_path).await?;

            for addr in addr_list {
                a_out.write(format!("ws://{addr}\n")).await?;
                println!("# HC SIGNAL - ADDR: ws://{addr}");
            }

            a_out.close().await?;

            println!("# HC SIGNAL - RUNNING");

            Some(sig_hnd)
        } else {
            None
        };

        tokio::select! {
            _ = bootstrap_shutdown_rx => (),
            _ = tokio::signal::ctrl_c() => (),
        }

        drop(sig_hnd);

        Ok(())
    }
}

fn tx_addr(addr: std::net::SocketAddr) -> Result<Vec<std::net::SocketAddr>> {
    if addr.ip().is_unspecified() {
        let port = addr.port();
        let mut list = Vec::new();
        let include_v6 = addr.ip().is_ipv6();

        for iface in if_addrs::get_if_addrs()? {
            if iface.ip().is_ipv6() && !include_v6 {
                continue;
            }
            list.push((iface.ip(), port).into());
        }

        Ok(list)
    } else {
        Ok(vec![addr])
    }
}
