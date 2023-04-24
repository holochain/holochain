use structopt::StructOpt;
use tokio::io::AsyncWriteExt;
use tx5_signal_srv::Result;

#[derive(Debug, StructOpt)]
/// Helper for running a holochain webrtc signal server.
pub struct HcSignalSrv {
    /// The port to use for the signal server. You probably want
    /// to leave this as 0 (zero) to be assigned an available port.
    #[structopt(short, long, default_value = "0")]
    port: u16,

    /// A comma separated list of interfaces on which to run the signal server.
    #[structopt(short, long, default_value = "127.0.0.1")]
    interfaces: String,

    /// If set, write the bound address list to a new file, separated by
    /// newlines. If the file exists, an error will be returned.
    #[structopt(long)]
    address_list_file_path: Option<std::path::PathBuf>,
}

impl HcSignalSrv {
    pub async fn run(self) {
        if let Err(err) = self.run_err().await {
            eprintln!("We were not able to start the signal server.");
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
            tracing::error!(?err);
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
