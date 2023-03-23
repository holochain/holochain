use holochain_cli_bundle::HcWebAppBundle;
use clap::Parser;

/// Main `hc-web-app` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcWebAppBundle::parse().run().await
}
