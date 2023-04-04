use clap::Parser;
use holochain_cli_bundle::HcAppBundle;

/// Main `hc-app` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcAppBundle::parse().subcommand.run().await
}
