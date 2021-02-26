use holochain_cli_bundle::HcDnaBundle;
use structopt::StructOpt;

/// Main `hc-dna` executable entrypoint.
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    HcDnaBundle::from_args().run().await
}
