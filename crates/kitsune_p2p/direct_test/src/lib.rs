use kitsune_p2p_direct::dependencies::*;
use kitsune_p2p_direct::*;

pub(crate) mod commands;

/// Option Parsing
#[derive(structopt::StructOpt, Debug)]
#[structopt(name = "kitsune-p2p-direct-test")]
pub struct Opt {
    #[structopt(subcommand)]
    cmd: OptCmd,

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
}

/// Subcommands
#[derive(structopt::StructOpt, Debug)]
pub enum OptCmd {
    /// Set up a group of periodically publishing nodes
    /// && count accessibility to data.
    BasicGossip {
        /// Each node will publish a new entry at this interval.
        #[structopt(short, long, default_value = "5000")]
        publish_interval_ms: u64,
    },
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
pub async fn execute(opt: Opt) -> KdResult<()> {
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

    // execute the specific test desired by the end user
    match opt.cmd {
        OptCmd::BasicGossip {
            publish_interval_ms,
        } => {
            commands::basic_gossip::execute(test_setup, publish_interval_ms).await?;
        }
    }

    Ok(())
}
