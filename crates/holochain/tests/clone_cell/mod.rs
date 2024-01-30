use holochain::sweettest::*;
use holochain_conductor_api::CellInfo;
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

    let apps = conductor.list_apps(None).await.unwrap();
    assert_eq!(1, apps.len());

    let cell_infos = apps
        .first()
        .unwrap()
        .cell_info
        .get(&dna_file.dna_hash().to_string())
        .unwrap();
    assert_eq!(2, cell_infos.len());
    assert_eq!(
        1,
        cell_infos
            .into_iter()
            .filter(|c| matches!(c, CellInfo::Cloned(_)))
            .count()
    );
}
