use holo_hash::ActionHash;
use holochain::sweettest::{SweetConductor, SweetDnaFile, SweetAgents};
use holochain_wasm_test_utils::TestWasm;


// Intended to keep https://github.com/holochain/holochain/issues/2541 fixed.
#[tokio::test(flavor = "multi_thread")]
async fn zome_with_no_link_types_does_not_prevent_deletes() {
    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::ValidateInvalid, TestWasm::Crd]).await;
    let agent = SweetAgents::alice();

    let (cell,) = conductor.setup_app_for_agent("app", agent, [&dna_file]).await.unwrap().into_tuple();

    let created: ActionHash = conductor.call(&cell.zome("crd"), "create", ()).await;
    
    let _: ActionHash = conductor.call(&cell.zome("crd"), "delete_via_hash", created).await;
}
