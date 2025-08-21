use crate::sweettest::{SweetConductor, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn request_dna_def() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cells = app.into_cells();

    let dna_def = conductor.get_dna_def(cells[0].cell_id());

    assert!(dna_def.is_some());
    assert!(dna_def.unwrap() == *dna.dna_def());
}
