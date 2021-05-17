use kitsune_p2p_direct::prelude::*;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct KdOptProxy {}

#[derive(Debug, StructOpt)]
struct KdOptNode {
    /// You must specify a proxy address to connect to
    proxy_url: String,
}

#[derive(Debug, StructOpt)]
enum KdOptCmd {
    /// Run a KitsuneDirect compatible proxy server.
    Proxy(KdOptProxy),

    /// Run a KitsuneDirect node.
    Node(KdOptNode),
}

#[derive(Debug, StructOpt)]
#[structopt(name = "kd", about = "Kitsune Direct Control CLI")]
struct KdOpt {
    #[structopt(subcommand)]
    cmd: KdOptCmd,
}

mod cmd_node;
mod cmd_proxy;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> KitsuneResult<()> {
    let KdOpt { cmd } = KdOpt::from_args();

    match cmd {
        KdOptCmd::Proxy(proxy_opt) => {
            cmd_proxy::run(proxy_opt).await?;
        }
        KdOptCmd::Node(node_opt) => {
            cmd_node::run(node_opt).await?;
        }
    }

    Ok(())
}
