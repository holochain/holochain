use holochain_diagnostics::{
    holochain::{conductor::conductor::RwShare, prelude::*, sweettest::*},
    ui::*,
    *,
};
use std::{
    collections::HashMap,
    error::Error,
    io::{self},
    sync::Arc,
    time::{Duration, Instant},
};
use tui::{backend::Backend, widgets::*, Terminal};

//                                        █████
//                                       ░░███
//   ██████   ██████  ████████    █████  ███████    █████
//  ███░░███ ███░░███░░███░░███  ███░░  ░░░███░    ███░░
// ░███ ░░░ ░███ ░███ ░███ ░███ ░░█████   ░███    ░░█████
// ░███  ███░███ ░███ ░███ ░███  ░░░░███  ░███ ███ ░░░░███
// ░░██████ ░░██████  ████ █████ ██████   ░░█████  ██████
//  ░░░░░░   ░░░░░░  ░░░░ ░░░░░ ░░░░░░     ░░░░░  ░░░░░░
//
//

const NODES: usize = 3;
const BASES: usize = 3;

const ENTRY_SIZE: usize = 1_00_000;
const MAX_COMMITS: usize = 100;

const REFRESH_RATE: Duration = Duration::from_millis(50);
const COMMIT_RATE: Duration = Duration::from_millis(5000);
const GET_RATE: Duration = Duration::from_millis(100);

const UI: bool = false;

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
    let app = setup_app().await;

    task_commit(app.clone());
    task_get(app.clone());

    if UI {
        tui_crossterm_setup(|t| run_app(t, app))?;
    } else {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await
        }
    }

    Ok(())
}

#[derive(Clone)]
struct App {
    state: RwShare<State<NODES, BASES>>,
    // start_time: Instant,
    ui: Ui<NODES, BASES>,
    bases: [AnyLinkableHash; BASES],
    // nodes: [Node; NODES],
    // agent_node_index: HashMap<AgentPubKey, usize>,
}

async fn setup_app() -> App {
    assert!(BASES <= NODES);
    // let config = config_historical_and_agent_gossip_only();
    let config = config_historical_only();
    // let config = config_standard();

    let (conductors, zomes) = diagnostic_tests::setup_conductors_single_zome(
        NODES,
        config,
        diagnostic_tests::basic_zome(),
    )
    .await;

    conductors.exchange_peer_info().await;
    println!("Peer info exchanged. Starting UI.");

    let mut nodes = vec![];

    for (conductor, zome) in std::iter::zip(conductors.into_iter().map(Arc::new), zomes.into_iter())
    {
        let dna_hash = zome.cell_id().dna_hash().clone();
        let diagnostics = conductor
            .holochain_p2p()
            .get_diagnostics(dna_hash)
            .await
            .unwrap();
        nodes.push(Node {
            conductor,
            zome,
            diagnostics,
        });
    }
    let bases = nodes
        .iter()
        .take(BASES)
        .map(|n| n.agent().into())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let nodes: [Node; NODES] = nodes.try_into().unwrap();

    let now = Instant::now();
    let commits = [0; BASES];
    let counts = [[(0, now); BASES]; NODES];

    let mut list_state: ListState = Default::default();
    list_state.select(Some(1));

    let state = RwShare::new(State {
        commits,
        counts,
        list_state,
    });
    let ui = Ui::new(nodes.clone(), now, REFRESH_RATE, state.clone());

    App {
        bases,
        // start_time: now,
        state,
        ui,
    }
}

fn task_get(app: App) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut i = 0;
        let mut last_zero = None;

        loop {
            let n = (i / BASES) % NODES;
            let b = i % BASES;

            let base = app.bases[b].clone();
            let links: usize = app.ui.nodes[n]
                .conductor
                .call(&app.ui.nodes[n].zome, "link_count", base)
                .await;

            let is_zero = app.state.share_mut(|state| {
                let val = state.commits[b] - links;
                state.counts[n][b].0 = val;
                state.counts[n][b].1 = Instant::now();
                val == 0
            });

            if is_zero {
                if let Some(last) = last_zero {
                    if i - last > NODES * BASES * 2 {
                        // If we've gone through two cycles of consistent zeros, then we can stop running get.
                        break;
                    }
                } else {
                    last_zero = Some(i);
                }
            } else {
                last_zero = None;
            }

            i += 1;

            tokio::time::sleep(GET_RATE).await;
        }
    })
}

fn task_commit(app: App) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut rng = seeded_rng(None);

        loop {
            let n = rng.gen_range(0..NODES);
            let b = rng.gen_range(0..BASES);

            let base = app.bases[b].clone();
            let _: ActionHash = app.ui.nodes[n]
                .conductor
                .call(
                    &app.ui.nodes[n].zome,
                    "create",
                    (base, random_vec::<u8>(&mut rng, ENTRY_SIZE)),
                )
                .await;

            let done = app.state.share_mut(|state| {
                state.commits[b] += 1;
                state.total_commits() >= MAX_COMMITS
            });
            if done {
                break;
            }
            tokio::time::sleep(COMMIT_RATE).await;
        }
    })
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| app.ui.render(f)).unwrap();
        if app.ui.input() {
            break;
        }
    }
    Ok(())
}
