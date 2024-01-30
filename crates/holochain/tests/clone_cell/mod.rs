use holochain::sweettest::*;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

#[tokio::test(flavor = "multi_thread")]
async fn create_clone_cell() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Clone]).await;

    let alice = SweetAgents::alice();

    let app = conductor
        .setup_app_for_agent("app", alice.clone(), [&dna_file])
        .await
        .unwrap();
    let (cell,) = app.clone().into_tuple();

    let zome = SweetZome::new(
        cell.cell_id().clone(),
        TestWasm::Clone.coordinator_zome_name(),
    );
    let request = CreateCloneCellInput {
        app_id: app.installed_app_id().clone(),
        role_name: dna_file.dna_hash().to_string(),
        modifiers: DnaModifiersOpt::none().with_network_seed("clone 1".to_string()),
        membrane_proof: None,
        name: Some("Clone 1".to_string()),
    };
    let _: ClonedCell = conductor.call(&zome, "create_clone", request).await;

    let app = conductor.get_app(app.installed_app_id()).await.unwrap();
    assert_eq!(1, app.clone_cells().count());
}
