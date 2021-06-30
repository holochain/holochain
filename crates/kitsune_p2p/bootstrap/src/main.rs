use std::net::IpAddr;

use structopt::StructOpt;

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

    let (jh, addr) = kitsune_p2p_bootstrap::run((
        bind.parse::<IpAddr>().expect("Failed to parse address"),
        port,
    ))
    .await
    .unwrap();

    println!("Connected to {:?}", addr);
    jh.await;
}
