use clap::Parser;
use holochain_cli as hc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // This is necessary to ensure deprecation warnings in holochain_cli_bundle are displayed by default
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "holochain_cli_bundle=warn");
    }
    holochain_trace::init_fmt(holochain_trace::Output::Log).ok();

    let cli = hc::Cli::parse();
    cli.subcommand.run().await
}
