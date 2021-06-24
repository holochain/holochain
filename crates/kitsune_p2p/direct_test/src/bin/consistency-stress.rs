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

    /// by default this executable injects bad agent info.
    /// if you wish to disable this, specify --no-bad-agent-infos.
    #[structopt(short = "b", long)]
    no_bad_agent_infos: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let Opt {
        node_count,
        agents_per_node,
        no_bad_agent_infos,
    } = Opt::from_args();

    let (mut progress, _shutdown) = run(Config {
        tuning_params: Default::default(),
        node_count,
        agents_per_node,
        bad_agent_infos: !no_bad_agent_infos,
    });

    while let Some(progress) = progress.next().await {
        println!("{}", progress);
    }
}
