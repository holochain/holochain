use kitsune_p2p_direct::dependencies::*;
use kitsune_p2p_direct::*;
use structopt::StructOpt;

#[tokio::main]
async fn main() {
    observability::init_fmt(observability::Output::LogTimed).ok();

    let opt = Opt::from_args();

    if let Err(e) = execute(opt).await {
        eprintln!("{:?}", e);
    }
}

/// Option Parsing
#[derive(structopt::StructOpt, Debug)]
#[structopt(name = "basic_gossip")]
struct Opt {
    /// Kitsune-proxy Url to connect to.
    #[structopt(
        short,
        long,
        default_value = "kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/proxy.holochain.org/p/5778/--"
    )]
    pub proxy_url: String,

    /// How many nodes to launch for this test.
    #[structopt(short, long, default_value = "32")]
    pub node_count: usize,

    /// Work directory for node persistence. [default: current-dir]
    #[structopt(short, long)]
    pub work_dir: Option<std::path::PathBuf>,

    /// Each node will publish a new entry at this interval.
    #[structopt(short, long, default_value = "5000")]
    publish_interval_ms: u64,
}

struct Node {
    node: KitsuneDirect,
    agent: KdHash,
}

impl Node {
    async fn new(config: KdConfig) -> KdResult<Self> {
        let node = spawn_kitsune_p2p_direct(config).await?;

        let agent = node.generate_agent().await?;

        Ok(Node { node, agent })
    }
}

struct TestSetup {
    root_node: Node,
    test_nodes: Vec<Node>,
}

/// Execute kitsune-p2p-direct test
async fn execute(opt: Opt) -> KdResult<()> {
    // generate our config
    // NOTE - this is very wrong, need to:
    //      - have a persist path
    //      - use quic
    //      - use proxy server
    let config = KdConfig {
        persist_path: None,
        unlock_passphrase: sodoken::Buffer::new(0),
        directives: vec![
            "set_proxy_accept_all:".to_string(),
            "bind_mem_local:".to_string(),
        ],
    };

    // generate our root node (root agent)
    let root_node = Node::new(config.clone()).await?;
    let test_nodes =
        futures::future::try_join_all((0..opt.node_count).map(|_| Node::new(config.clone())))
            .await?;

    // all the nodes should join the root app
    let root_agent = root_node.agent.clone();
    root_node
        .node
        .join(root_agent.clone(), root_agent.clone())
        .await?;
    futures::future::try_join_all(
        test_nodes
            .iter()
            .map(|n| n.node.join(root_agent.clone(), n.agent.clone())),
    )
    .await?;

    // collect connection info from all nodes
    let mut agent_info = futures::future::try_join_all(
        test_nodes
            .iter()
            .map(|n| n.node.list_known_agent_info(root_agent.clone())),
    )
    .await?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    agent_info.append(
        &mut root_node
            .node
            .list_known_agent_info(root_agent.clone())
            .await?,
    );

    // share connection info to all nodes
    root_node
        .node
        .inject_agent_info(root_agent.clone(), agent_info.clone())
        .await?;
    futures::future::try_join_all(test_nodes.iter().map(|n| {
        n.node
            .inject_agent_info(root_agent.clone(), agent_info.clone())
    }))
    .await?;

    // construct the shared test setup struct
    let test_setup = TestSetup {
        root_node,
        test_nodes,
    };

    execute_basic_gossip(test_setup, opt.publish_interval_ms).await?;

    Ok(())
}

async fn execute_basic_gossip(test_setup: TestSetup, publish_interval_ms: u64) -> KdResult<()> {
    let mut total_pub_count = 0_u64;
    let root_agent = test_setup.root_node.agent.clone();
    loop {
        let mut avg_store_count = 0.0_f64;
        let mut store_count_count = 0.0_f64;
        for node in test_setup.test_nodes.iter() {
            avg_store_count += node
                .node
                .list_left_links(root_agent.clone(), root_agent.clone())
                .await?
                .into_iter()
                .count() as f64;
            store_count_count += 1.0;
        }

        println!(
            "{} / {} (Avg Store Count / Total Publish Count)",
            avg_store_count / store_count_count,
            total_pub_count,
        );

        for node in test_setup.test_nodes.iter() {
            node.node
                .create_entry(
                    root_agent.clone(),
                    node.agent.clone(),
                    KdEntry::builder()
                        .set_sys_type(SysType::Create)
                        .set_expire(chrono::MAX_DATETIME)
                        .set_left_link(&root_agent),
                )
                .await?;
            total_pub_count += 1;
        }

        tokio::time::delay_for(std::time::Duration::from_millis(publish_interval_ms)).await;
    }
}
