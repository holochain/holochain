use futures::stream::StreamExt;
use kitsune_p2p_direct::dependencies::*;
use kitsune_p2p_direct_test::consistency_stress::*;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "consistency-stress")]
struct Opt {
    /// how many nodes to create
    #[structopt(short = "n", long, default_value = "10")]
    node_count: usize,

    /// how many agents to join on each node
    #[structopt(short = "a", long, default_value = "10")]
    agents_per_node: usize,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let Opt {
        node_count,
        agents_per_node,
    } = Opt::from_args();

    let (mut progress, _shutdown) = run(Config {
        node_count,
        agents_per_node,
    });

    while let Some(progress) = progress.next().await {
        println!("{}", progress);
    }
}
