//! Simulate behavior of a typical Syn app

use std::{sync::Arc, time::Duration};

use diagnostic_tests::{setup_conductors_single_zome, syn_zome};
use holo_hash::AgentPubKey;
use holochain_diagnostics::{holochain::sweettest::*, Signal};
use tokio_stream::{Stream, StreamExt, StreamMap};

const NODES: usize = 5;

#[tokio::main]
async fn main() {
    let (app, signal_rxs) = App::setup().await;

    let mut handles = vec![];
    handles.push(task_signal_handler(app.clone(), signal_rxs));
    handles.push(task_signal_sender(app.clone()));

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
        let (mut conductors, zomes) = setup_conductors_single_zome(NODES, config, syn_zome()).await;

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
    tokio::spawn(async move {})
}

fn task_signal_sender(app: App) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let n = &app.nodes[0];
            let ps: Vec<AgentPubKey> = (0..NODES)
                .into_iter()
                .map(|p| app.nodes[p].zome.cell_id().agent_pubkey().clone())
                .collect();
            println!("sending message to {} agents", ps.len());

            let _: () = n
                .conductor
                .call(&n.zome, "send_message", (vec![123], ps))
                .await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}

fn task_signal_handler(app: App, signal_rxs: Vec<SignalStream>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut streams = StreamMap::new();
        for (i, s) in signal_rxs.into_iter().enumerate() {
            streams.insert(i, s);
        }
        loop {
            println!("awaiting signal");
            if let Some((i, signal)) = streams.next().await {
                println!("got signal: {} {:?}", i, signal);
            } else {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    })
}
