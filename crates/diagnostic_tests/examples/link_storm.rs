use holochain_diagnostics::{
    holochain::{
        conductor::{conductor::RwShare, config::ConductorConfig},
        prelude::*,
        sweettest::*,
    },
    ui::gossip_dashboard::*,
    *,
};
use std::{
    collections::HashMap,
    error::Error,
    io::{self},
    sync::Arc,
    time::{Duration, Instant},
};
use tui::{backend::Backend, Terminal};

const BASES: usize = 12;

const ENTRY_SIZE: usize = 1_000_000;
const MAX_COMMITS: usize = 1_000;
const ENTRIES_PER_COMMIT: u32 = 100;

const COMMIT_RATE: Duration = Duration::from_millis(0);
const GET_RATE: Duration = Duration::from_millis(100);

const REFRESH_RATE: Duration = Duration::from_millis(50);

/// Display the UI if all other conditions are met
const UI: bool = true;

/// Config for each conductor
fn config() -> ConductorConfig {
    // config_historical_and_agent_gossip_only()
    // config_recent_only()
    // config_historical_only()
    // config_standard()

    let mut config = config_standard();
    config.network.as_mut().map(|c| {
        *c = c.clone().tune(|mut tp| {
            tp.disable_publish = true;
            // tp.disable_historical_gossip = true;
            tp.danger_gossip_recent_threshold_secs = 5;

            tp.gossip_inbound_target_mbps = 1000000.0;
            tp.gossip_outbound_target_mbps = 1000000.0;
            tp.gossip_historic_outbound_target_mbps = 1000000.0;
            tp.gossip_historic_inbound_target_mbps = 1000000.0;
            tp
        });
    });
    config
}

//                             ███
//                            ░░░
//  █████████████    ██████   ████  ████████
// ░░███░░███░░███  ░░░░░███ ░░███ ░░███░░███
//  ░███ ░███ ░███   ███████  ░███  ░███ ░███
//  ░███ ░███ ░███  ███░░███  ░███  ░███ ░███
//  █████░███ █████░░████████ █████ ████ █████
// ░░░░░ ░░░ ░░░░░  ░░░░░░░░ ░░░░░ ░░░░ ░░░░░
//
//
//

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    observability::test_run().ok();

    println!(
        "Total amount of entry data to commit: {}",
        (MAX_COMMITS * ENTRY_SIZE).human_count_bytes()
    );

    let app = setup_app(seeded_rng(None)).await;

    let yes_ui = std::env::var("NOUI").is_err();
    let show_ui = UI && std::env::var("RUST_LOG").is_err() && yes_ui;

    let commit_task = spawn_commit_task(app.clone());
    let get_task = spawn_get_task(app.clone());

    let tasks = futures::future::join_all([commit_task, get_task]);

    if show_ui {
        let ui_task = tokio::task::spawn_blocking(|| tui_crossterm_setup(|t| run_app(t, app)));
        tokio::select! {
            r = tasks => { r.into_iter().collect::<Result<Vec<_>, _>>().unwrap(); }
            r = ui_task => { r.unwrap().unwrap() }
        }
    } else {
        tokio::select! {
            r = tasks => { r.into_iter().collect::<Result<Vec<_>, _>>().unwrap();  }
        }
    }

    Ok(())
}

//                    █████
//                   ░░███
//   █████   ██████  ███████   █████ ████ ████████
//  ███░░   ███░░███░░░███░   ░░███ ░███ ░░███░░███
// ░░█████ ░███████   ░███     ░███ ░███  ░███ ░███
//  ░░░░███░███░░░    ░███ ███ ░███ ░███  ░███ ░███
//  ██████ ░░██████   ░░█████  ░░████████ ░███████
// ░░░░░░   ░░░░░░     ░░░░░    ░░░░░░░░  ░███░░░
//                                        ░███
//                                        █████
//                                       ░░░░░

async fn setup_app(mut rng: StdRng) -> App {
    let zome = diagnostic_tests::basic_zome();
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(("zome", zome)).await;
    let bases = (0..BASES)
        .map(|_| ActionHash::from_raw_32(random_vec(&mut rng, 32)).into())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    let commits = [0; BASES];

    let mut state = State::new(commits, rng);
    state.add_node(construct_node(dna.clone()).await);
    state.add_node(construct_node(dna.clone()).await);
    let state = RwShare::new(state);
    let ui = GossipDashboard::new(Some(0), Instant::now(), REFRESH_RATE);

    let app = App {
        bases,
        state,
        ui,
        dna,
    };
    exchange_all_peers(app.clone()).await;

    app
}

async fn construct_node(dna: DnaFile) -> Node {
    let (conductor, zome) = diagnostic_tests::setup_conductor_with_single_dna(config(), dna).await;
    let conductor = Arc::new(conductor);
    let node = Node::new(conductor.clone(), zome).await;
    node
}

async fn introduce_node_to_peers(node: &Node, peers: &[Node]) {
    if !peers.is_empty() {
        futures::future::join_all(peers.iter().map(|peer| async move {
            SweetConductor::exchange_peer_info([&peer.conductor, &*node.conductor]).await;
            peer.conductor
                .holochain_p2p()
                .new_integrated_data(peer.zome.cell_id().dna_hash().clone())
                .await
                .unwrap();
            dbg!(peer
                .conductor
                .get_agent_infos(Some(peer.zome.cell_id().clone()))
                .await
                .unwrap());
        }))
        .await;
    }
}

async fn exchange_all_peers(app: App) {
    let cs: Vec<_> = app
        .state
        .share_ref(|state| state.nodes().iter().map(|n| n.conductor.clone()).collect());
    SweetConductor::exchange_peer_info(cs.iter().map(|c| &**c)).await;
}

//   █████
//  ░░███
//  ███████   █████ ████ ████████   ██████   █████
// ░░░███░   ░░███ ░███ ░░███░░███ ███░░███ ███░░
//   ░███     ░███ ░███  ░███ ░███░███████ ░░█████
//   ░███ ███ ░███ ░███  ░███ ░███░███░░░   ░░░░███
//   ░░█████  ░░███████  ░███████ ░░██████  ██████
//    ░░░░░    ░░░░░███  ░███░░░   ░░░░░░  ░░░░░░
//             ███ ░███  ░███
//            ░░██████   █████
//             ░░░░░░   ░░░░░

pub type Base = AnyLinkableHash;

#[derive(Clone)]
struct App {
    state: RwShare<State>,
    ui: GossipDashboard,
    bases: [Base; BASES],
    dna: DnaFile,
}

struct State {
    time: Instant,
    nodes: Vec<Node>,
    commits: [usize; BASES],
    link_counts: LinkCounts,
    rng: StdRng,
    done_time: Option<Instant>,

    /// Cached reverse lookup for node index by agent key.
    /// Must be in sync with `nodes`!
    agent_node_index: HashMap<AgentPubKey, usize>,
}

impl ClientState for State {
    fn time(&self) -> Instant {
        self.time
    }

    fn num_bases(&self) -> usize {
        BASES
    }

    fn nodes(&self) -> &[Node] {
        self.nodes.as_slice()
    }

    fn total_commits(&self) -> usize {
        self.commits.iter().sum()
    }

    fn link_counts(&self) -> LinkCountsRef {
        self.link_counts.as_ref()
    }

    fn node_histories_sorted<'a>(&self, metrics: &'a metrics::Metrics) -> NodeHistories<'a, usize> {
        let mut histories: Vec<_> = metrics
            .peer_node_histories()
            .values()
            .filter_map(|history| {
                assert!(
                    history.remote_agents.len() <= 1,
                    "this widget cannot be used with more than 1 agent per node"
                );
                history.remote_agents.first().map(|agent| {
                    (
                        *self
                            .agent_node_index
                            .get(&AgentPubKey::from_kitsune(agent))
                            .unwrap(),
                        history,
                    )
                })
            })
            .collect();
        histories.sort_unstable_by_key(|(i, _)| *i);
        histories
    }
}

impl State {
    fn new(commits: [usize; BASES], rng: StdRng) -> Self {
        let state = Self {
            time: Instant::now(),
            commits,
            rng,
            nodes: Default::default(),
            link_counts: Default::default(),
            agent_node_index: Default::default(),
            done_time: Default::default(),
        };
        state
    }

    fn add_node(&mut self, node: Node) {
        let new_index = self.nodes.len();
        self.link_counts
            .push(vec![(0, Instant::now()); self.num_bases()]);
        self.agent_node_index
            .insert(node.zome.cell_id().agent_pubkey().clone(), new_index);
        self.nodes.push(node);
    }
}

//   █████                      █████
//  ░░███                      ░░███
//  ███████    ██████    █████  ░███ █████  █████
// ░░░███░    ░░░░░███  ███░░   ░███░░███  ███░░
//   ░███      ███████ ░░█████  ░██████░  ░░█████
//   ░███ ███ ███░░███  ░░░░███ ░███░░███  ░░░░███
//   ░░█████ ░░████████ ██████  ████ █████ ██████
//    ░░░░░   ░░░░░░░░ ░░░░░░  ░░░░ ░░░░░ ░░░░░░
//
//
//

fn spawn_get_task(app: App) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut i = 0;
        let mut last_zero = None;

        loop {
            let b = i % BASES;
            let base = app.bases[b].clone();

            let (n, node, num_nodes) = app.state.share_ref(|state| {
                let num_nodes = state.nodes.len();
                let n = (i / BASES) % num_nodes;
                let node = state.nodes[n].clone();
                (n, node, num_nodes)
            });

            let links: usize = node
                .conductor
                .call(&node.zome, "link_count", (base, false))
                .await;

            app.state.share_mut(|state| {
                let val = state.commits[b] - links;
                state.link_counts[n][b].0 = val;
                state.link_counts[n][b].1 = Instant::now();
                let (is_zero, is_done) = (val == 0, state.total_commits() >= MAX_COMMITS);

                // Keep track of when we got to all zeros
                if is_zero {
                    if let Some(last) = last_zero {
                        if is_done && i - last > num_nodes * BASES * 2 {
                            state.done_time = Some(Instant::now());
                        }
                    } else {
                        last_zero = Some(i);
                    }
                } else {
                    if last_zero.is_some() {
                        state.done_time = None;
                        last_zero = None;
                    }
                }
                state.time = Instant::now();
            });

            i += 1;

            tokio::time::sleep(GET_RATE).await;
        }
    })
}

async fn create_new_node(app: App, selected_node: usize) {
    // TODO: get actual selected node
    let node = construct_node(app.dna.clone()).await;
    let peers: Vec<_> = app.state.share_ref(|state| {
        [selected_node]
            .iter()
            .map(|p| state.nodes[*p].clone())
            .collect()
    });

    introduce_node_to_peers(&node, &peers).await;

    app.state.share_mut(|state| {
        state.add_node(node);
    })
}

fn random_node(state: &mut State) -> &Node {
    let num = state.nodes.len();
    assert!(num > 0);
    let n = state.rng.gen_range(0..num);
    &state.nodes[n]
}

// fn random_base(rng: &mut StdRng, app: &App) -> &Base {
//     let b = rng.gen_range(0..BASES);
//     &app.bases[b]
// }

async fn commit_random(app: &App) -> usize {
    let (node, base_index) = app
        .state
        .share_mut(|state| (random_node(state).clone(), state.rng.gen_range(0..BASES)));
    commit(app, &node, base_index).await
}

async fn commit(app: &App, node: &Node, base_index: usize) -> usize {
    let base = app.bases[base_index].clone();
    let _: () = node
        .conductor
        .call(
            &node.zome,
            "create_batch_random",
            (base, ENTRIES_PER_COMMIT, ENTRY_SIZE),
        )
        .await;

    let total = app.state.share_mut(|state| {
        state.commits[base_index] += ENTRIES_PER_COMMIT as usize;
        state.total_commits()
    });

    total
}

fn spawn_commit_task(app: App) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let total = commit_random(&app).await;
            if total >= MAX_COMMITS {
                break;
            } else {
                tokio::time::sleep(COMMIT_RATE).await;
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    })
}

fn run_app<B: Backend + io::Write>(terminal: &mut Terminal<B>, app: App) -> io::Result<()> {
    loop {
        let cmd = app.ui.input(app.state.clone());
        match cmd {
            Some(InputCmd::Quit) => break,
            Some(InputCmd::ClearBuffer) => {
                exit_tui(terminal.backend_mut())?;
                terminal.draw(|f| app.ui.clear(f))?;
                enter_tui(&mut io::stdout())?;
            }
            Some(InputCmd::ExchangePeers) => {
                tokio::spawn(exchange_all_peers(app.clone()));
            }
            Some(InputCmd::AddNode(index)) => {
                tokio::spawn(create_new_node(app.clone(), index));
            }
            None => (),
        };

        let _ = app
            .state
            .share_ref(|state| terminal.draw(|f| app.ui.render(f, state)).unwrap());
    }
    Ok(())
}
