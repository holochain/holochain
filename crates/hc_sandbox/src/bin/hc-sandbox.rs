use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var_os("RUST_LOG").is_some() {
        holochain_trace::init_fmt(holochain_trace::Output::Log).ok();
    }
    let ops = holochain_cli_sandbox::HcSandbox::parse();

    ops.run().await
}
