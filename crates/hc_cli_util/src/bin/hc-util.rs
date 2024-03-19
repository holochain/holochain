use clap::Parser;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    if std::env::var_os("RUST_LOG").is_some() {
        holochain_trace::init_fmt(holochain_trace::Output::Log).ok();
    }
    let ops = holochain_cli_util::Util::parse();

    ops.run().await
}
