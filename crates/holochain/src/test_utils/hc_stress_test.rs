//! automated behavioral testing of hc-stress-test zomes

use crate::sweettest::*;
use holochain_wasm_test_utils::*;
use holochain_types::prelude::*;

fn err_other<E>(error: E) -> std::io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    std::io::Error::new(std::io::ErrorKind::Other, error.into())
}

/// run a test involving only two nodes
pub async fn hc_stress_test_two_nodes(
) -> std::io::Result<()> {
    let mut conductor = SweetConductor::from_standard_config().await;
    let network_seed = random_network_seed();
    let _app = install(network_seed, &mut conductor).await;

    Err(err_other("hello"))
}

async fn install(network_seed: String, conductor: &mut SweetConductor) -> SweetApp {
    let (dna, _, _) = SweetDnaFile::from_zomes(
        network_seed,
        vec![TestIntegrityWasm::HcStressTestIntegrity],
        vec![TestCoordinatorWasm::HcStressTestCoordinator],
        vec![
            DnaWasm::from(TestIntegrityWasm::HcStressTestIntegrity),
            DnaWasm::from(TestCoordinatorWasm::HcStressTestCoordinator),
        ],
        SerializedBytes::default(),
    ).await;
    let _dna_hash = dna.dna_hash().clone();
    conductor.setup_app("app", &[dna]).await.unwrap()
}
