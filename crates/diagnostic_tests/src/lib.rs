use std::time::Instant;

use holochain_diagnostics::holochain::conductor::config::ConductorConfig;
use holochain_diagnostics::holochain::prelude::*;
use holochain_diagnostics::holochain::sweettest::*;

mod zomes;
pub use zomes::*;

pub async fn setup_conductor_with_single_dna(
    config: ConductorConfig,
    dna: DnaFile,
) -> (SweetConductor, SweetZome) {
    let mut conductor = SweetConductor::from_config(config).await;
    let app = conductor.setup_app("basic", &[dna]).await.unwrap();
    let (cell,) = app.into_tuple();
    let zome = cell.zome("zome");
    (conductor, zome)
}

pub async fn setup_conductors_with_single_dna(
    nodes: usize,
    config: ConductorConfig,
    dna: DnaFile,
) -> (SweetConductorBatch, Vec<SweetZome>) {
    let start = Instant::now();

    let mut conductors = SweetConductorBatch::from_config(nodes, config).await;
    println!("Conductors created (t={:3.1?}).", start.elapsed());

    let apps = conductors.setup_app("basic", &[dna]).await.unwrap();
    let cells = apps.cells_flattened().clone();
    println!("Apps setup (t={:3.1?}).", start.elapsed());
    println!(
        "agents: {:#?}",
        cells.iter().map(|c| c.agent_pubkey()).collect::<Vec<_>>()
    );

    let zomes = cells.iter().map(|c| c.zome("zome")).collect::<Vec<_>>();

    (conductors, zomes)
}
