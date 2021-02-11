use holochain_cli_bundle::HcAppBundle;
use structopt::StructOpt;

/// Main `hc-app` executable entrypoint.
#[tokio::main]
pub async fn main() {
    let opt = HcAppBundle::from_args();
    if let Err(err) = opt.run().await {
        eprintln!("hc-app: {}", err);
        std::process::exit(1);
    }
}
