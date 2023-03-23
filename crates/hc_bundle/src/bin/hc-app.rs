use holochain_cli_bundle::HcAppBundle;
use clap::Parser;

/// Main `hc-app` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcAppBundle::parse().run().await
}
