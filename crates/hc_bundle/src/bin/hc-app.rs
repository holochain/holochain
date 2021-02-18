use holochain_cli_bundle::HcAppBundle;
use structopt::StructOpt;

/// Main `hc-app` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcAppBundle::from_args().run().await
}
