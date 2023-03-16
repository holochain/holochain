use holochain_cli_bundle::HcWebAppBundle;
use structopt::StructOpt;

/// Main `hc-web-app` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcWebAppBundle::from_args().run().await
}
