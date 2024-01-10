use clap::Parser;
use holochain_cli_admin::HcAdmin;

/// Main `hc-admin` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcAdmin::parse().run().await
}
