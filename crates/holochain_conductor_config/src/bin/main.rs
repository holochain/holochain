use clap::Parser;
use holochain_conductor_config::ConductorConfigCli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = ConductorConfigCli::parse();
    cli.run().await
}
