use clap::Parser;
use holochain_cli_bundle::HcDnaBundle;

/// Main `hc-dna` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcDnaBundle::parse().run().await
}
