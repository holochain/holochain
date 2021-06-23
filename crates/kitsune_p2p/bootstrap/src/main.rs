use std::net::IpAddr;

use structopt::StructOpt;
use tokio::sync::oneshot;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "kitsune-bootstrap",
    about = "Sever for bootstrapping kitsune nodes"
)]
struct Opt {
    /// Set port
    #[structopt(short, long, default_value = "0")]
    port: u16,
    /// Address to bind to.
    #[structopt(short, long, default_value = "127.0.0.1")]
    bind: String,
}

#[tokio::main]
async fn main() {
    let Opt { port, bind } = Opt::from_args();
    let (tx, rx) = oneshot::channel();
    let jh = tokio::task::spawn(kitsune_bootstrap::run(
        (
            bind.parse::<IpAddr>().expect("Failed to parse address"),
            port,
        ),
        tx,
    ));
    let addr = rx.await;
    if let Ok(addr) = addr {
        println!("Connected to {:?}", addr);
        jh.await.unwrap();
    }
}
