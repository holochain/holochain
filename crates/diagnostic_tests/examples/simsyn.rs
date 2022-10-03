//! Simulate behavior of a typical Syn app

use std::sync::Arc;

use holochain_diagnostics::holochain::sweettest::*;

#[tokio::main]
async fn main() {}

struct App {
    conductors: Arc<SweetConductorBatch>,
    zomes: Vec<SweetZome>,
}

fn task_commit() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {})
}

fn task_signal() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {})
}
