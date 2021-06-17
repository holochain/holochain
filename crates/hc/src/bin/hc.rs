use holochain_cli as hc;
use structopt::StructOpt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var_os("RUST_LOG").is_some() {
        observability::init_fmt(observability::Output::Log).ok();
    }
    let opt = hc::Opt::from_args();
    opt.run().await
}
