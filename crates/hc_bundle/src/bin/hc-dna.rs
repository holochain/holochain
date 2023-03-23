use holochain_cli_bundle::HcDnaBundle;
use clap::Parser;

/// Main `hc-dna` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcDnaBundle::parse().run().await
}
