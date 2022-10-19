//! Simulate behavior of a typical Syn app

use std::{
    io::Write,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use diagnostic_tests::syn_zome;
use holochain_diagnostics::{holochain::sweettest::*, random_vec, seeded_rng, AgentPubKey};
use tokio_stream::{StreamExt, StreamMap};

const NODES: usize = 10;
const COMMIT_SIZE: usize = 1_000_000;

const SEND_RATE: Duration = Duration::from_millis(2000);
const COMMIT_RATE: Duration = Duration::from_millis(2000);

static SIGNALS_SENT: AtomicUsize = AtomicUsize::new(0);

#[tokio::main]
async fn main() {
    let (app, signal_rxs) = App::setup().await;

    let mut handles = vec![];
    handles.push(task_signal_handler(app.clone(), signal_rxs));
    handles.push(task_signal_sender(app.clone()));
    handles.push(task_commit(app.clone()));

    futures::future::join_all(handles).await;
}

#[derive(Clone)]
struct Node {
    conductor: Arc<SweetConductor>,
    zome: SweetZome,
}

#[derive(Clone)]
struct App {
    nodes: Vec<Node>,
}

impl App {
    async fn setup() -> (Self, Vec<SignalStream>) {
        let config = standard_config();

        let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(("zome", syn_zome())).await;
        let (mut conductors, zomes) =
            diagnostic_tests::setup_conductors_single_dna(NODES, config, dna).await;

        conductors.exchange_peer_info().await;

        let signal_rxs = conductors.iter_mut().map(|c| c.signals()).collect();
        let nodes = std::iter::zip(conductors.into_iter().map(Arc::new), zomes.into_iter())
            .map(|(conductor, zome)| Node { conductor, zome })
            .collect();
        let app = Self { nodes };
        (app, signal_rxs)
    }
}

fn task_commit(app: App) -> tokio::task::JoinHandle<()> {
    let mut rng = seeded_rng(None);
    tokio::spawn(async move {
        let mut n = 0;
        loop {
            let node: &Node = &app.nodes[n % NODES];
            let data = random_vec::<u8>(&mut rng, COMMIT_SIZE);
            let _: () = node.conductor.call(&node.zome, "commit", data).await;

            println!(
                "\ncommitted. signals so far: {}",
                SIGNALS_SENT.load(Ordering::Relaxed)
            );

            n += 1;
            tokio::time::sleep(COMMIT_RATE).await;
        }
    })
}

fn task_signal_sender(app: App) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut n = 0;
        let agents: Vec<_> = app
            .nodes
            .iter()
            .map(|n| n.zome.cell_id().agent_pubkey().clone())
            .collect();
        loop {
            let node: &Node = &app.nodes[n % NODES];
            let ps: Vec<AgentPubKey> = agents
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != n)
                .map(|(_, p)| p.clone())
                .collect();

            SIGNALS_SENT.fetch_add(ps.len(), Ordering::Relaxed);

            let _: () = node
                .conductor
                .call(&node.zome, "send_message", (vec![123], ps))
                .await;

            n += 1;
            tokio::time::sleep(SEND_RATE).await;
        }
    })
}

fn task_signal_handler(_app: App, signal_rxs: Vec<SignalStream>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut streams = StreamMap::new();
        for (i, s) in signal_rxs.into_iter().enumerate() {
            streams.insert(i, s);
        }
        loop {
            if let Some((_i, _signal)) = streams.next().await {
                print!(".");
                std::io::stdout().flush().ok();
            } else {
                println!("No signal. Closing handler loop.");
                break;
            }
        }
    })
}
