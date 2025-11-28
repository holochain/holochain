use clap::Parser;
use holochain_cli_client::HcClient;

/// Entry point for the `hc-client` binary.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = HcClient::parse();
    cli.run().await
}
