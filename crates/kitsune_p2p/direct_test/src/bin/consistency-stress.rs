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

    /// introduce this number of bad agent infos every round
    #[structopt(short = "b", long, default_value = "1")]
    bad_agent_count: usize,

    /// have this many agents drop after each op consistency
    /// the same number of new agents will also be added
    #[structopt(short = "t", long, default_value = "1")]
    agent_turnover_count: usize,

    /// reconfigure the tuning_param that controls delay
    /// re-gossiping with a remote node after a successful gossip
    #[structopt(short = "d", long, default_value = "60000")]
    peer_gossip_success_delay_ms: u32,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let Opt {
        node_count,
        agents_per_node,
        bad_agent_count,
        agent_turnover_count,
        peer_gossip_success_delay_ms,
    } = Opt::from_args();

    let mut tuning_params =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning_params.gossip_peer_on_success_next_gossip_delay_ms = peer_gossip_success_delay_ms;
    let tuning_params = std::sync::Arc::new(tuning_params);

    let (mut progress, _shutdown) = run(Config {
        tuning_params,
        node_count,
        agents_per_node,
        bad_agent_count,
        agent_turnover_count,
    });

    while let Some(progress) = progress.next().await {
        println!("{}", progress);
    }
}
