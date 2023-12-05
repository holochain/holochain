use clap::Parser;
use holochain_cli_bundle::HcWebAppBundle;

/// Main `hc-web-app` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcWebAppBundle::parse().run().await
}
