use clap::Parser;
use tokio::io::AsyncWriteExt;
use tx5_signal_srv::Result;

#[derive(Debug, Parser)]
/// Run a Holochain WebRTC signal server.
pub struct HcSignalSrv {
    /// The port to use for the signal server. Defaults to 0, which autoselects an available high-range port.
    #[arg(short, long, default_value = "0")]
    port: u16,

    /// A comma separated list of interfaces to which to bind the signal server.
    #[arg(short, long, default_value = "127.0.0.1", value_delimiter = ',')]
    interfaces: String,

    /// If set, write the bound interface list to a new file, separated by
    /// newlines. If the file exists, an error will be returned.
    #[arg(long)]
    address_list_file_path: Option<std::path::PathBuf>,
}

impl HcSignalSrv {
    pub async fn run(self) {
        if let Err(err) = self.run_err().await {
            eprintln!("Unable to start the signal server.");
            eprintln!("{err:#?}");
        }
    }

    pub async fn run_err(self) -> Result<()> {
        let mut config = tx5_signal_srv::Config::default();
        config.interfaces = self.interfaces;
        config.port = self.port;
        config.demo = false;
        tracing::info!(?config);

        let (driver, addr_list, err_list) = tx5_signal_srv::exec_tx5_signal_srv(config)?;

        for err in err_list {
            println!("# HC SIGNAL SRV - ERROR: {err:?}");
        }

        if let Some(path) = &self.address_list_file_path {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .await?;

            for addr in &addr_list {
                file.write_all(format!("ws://{addr}\n").as_bytes()).await?;
            }

            file.flush().await?;
            file.shutdown().await?;
            drop(file);
        }

        for addr in addr_list {
            println!("# HC SIGNAL SRV - ADDR: ws://{addr}");
        }

        println!("# HC SIGNAL SRV - RUNNING");

        driver.await;

        Ok(())
    }
}
