use structopt::StructOpt;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    if std::env::var_os("RUST_LOG").is_some() {
        holochain_trace::init_fmt(holochain_trace::Output::Log).ok();
    }
    let ops = holochain_cli_signal_srv::HcSignalSrv::from_args();

    ops.run().await
}
