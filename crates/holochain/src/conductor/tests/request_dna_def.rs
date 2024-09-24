use holochain_wasm_test_utils::TestWasm;

use crate::sweettest::{SweetConductor, SweetDnaFile};

#[tokio::test(flavor = "multi_thread")]
async fn request_dna_def() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductor = SweetConductor::isolated_singleton().await;
    conductor.setup_app("app", [&dna]).await.unwrap();

    let dna_def = conductor.get_dna_def(dna.dna_hash());

    assert!(dna_def.is_some());
    assert!(dna_def.unwrap() == *dna.dna_def());
}
