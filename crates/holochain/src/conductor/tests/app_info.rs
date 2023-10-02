use holochain_conductor_api::CellInfo;
use holochain_types::prelude::{
    CloneCellId, CreateCloneCellPayload, DisableCloneCellPayload, EnableCloneCellPayload,
    InstalledAppId,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{DnaModifiersOpt, RoleName};
use matches::matches;

use crate::sweettest::{SweetAgents, SweetConductor, SweetDnaFile};

#[tokio::test(flavor = "multi_thread")]
async fn app_info_returns_all_cells_with_info() {
    // set up app with two provisioned cells and one clone cell of each of them
    let (dna_1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna_2, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let agent_pub_key = SweetAgents::one(conductor.keystore()).await;

    let app_id: InstalledAppId = "app".into();
    let role_name_1: RoleName = "role_1".into();
    let role_name_2: RoleName = "role_2".into();
    conductor
        .setup_app_for_agent(
            &app_id,
            agent_pub_key.clone(),
            [
                &(role_name_1.clone(), dna_1.clone()),
                &(role_name_2.clone(), dna_2.clone()),
            ],
        )
        .await
        .unwrap();

    // create 1 clone cell for role 1 = clone cell 1
    let clone_cell_1 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app_id.clone(),
            role_name: role_name_1.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed numero uno".to_string()),
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();
    assert_eq!(clone_cell_1.original_dna_hash, dna_1.dna_hash().clone());

    // create 1 clone cell for role 2 = clone cell 2
    let clone_cell_2 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app_id.clone(),
            role_name: role_name_2.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed numero dos".to_string()),
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();
    assert_eq!(clone_cell_2.original_dna_hash, dna_2.dna_hash().clone());

    // disable clone cell 2
    conductor
        .disable_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.clone(),
            clone_cell_id: CloneCellId::CellId(clone_cell_2.cell_id.clone()),
        })
        .await
        .unwrap();

    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();

    // agent pub key matches
    assert_eq!(app_info.agent_pub_key, agent_pub_key);

    // app info has cell info for two role names
    assert_eq!(app_info.cell_info.len(), 2);

    // check cell info for role name 1
    let cell_info_for_role_1 = app_info.cell_info.get(&role_name_1).unwrap();
    // cell 1 in cell info is provisioned cell
    matches!(cell_info_for_role_1[0], CellInfo::Provisioned(_));
    // cell 2 in cell info is clone cell
    matches!(cell_info_for_role_1[1], CellInfo::Cloned(_));

    // check cell info for role name 2
    let cell_info_for_role_2 = app_info.cell_info.get(&role_name_2).unwrap();
    // cell 1 in cell info is provisioned cell
    matches!(cell_info_for_role_2[0], CellInfo::Provisioned(_));
    // cell 2 in cell info is clone cell
    matches!(cell_info_for_role_2[1], CellInfo::Cloned(_));

    // clone cell ids match
    assert!(if let CellInfo::Cloned(cell) = &cell_info_for_role_1[1] {
        cell.cell_id == clone_cell_1.cell_id.clone()
    } else {
        false
    });

    assert!(if let CellInfo::Cloned(cell) = &cell_info_for_role_2[1] {
        cell.cell_id == clone_cell_2.cell_id.clone()
    } else {
        false
    });

    conductor.shutdown().await;
    conductor.startup().await;

    // make sure app info is identical after conductor restart
    let app_info_after_restart = conductor
        .clone()
        .get_app_info(&app_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(app_info, app_info_after_restart);

    // make sure the re-enabled clone cell's original DNA hash matches
    // tests that the enable_clone_cell fn returns the right DNA hash
    let reenabled_clone_cell = conductor
        .clone()
        .enable_clone_cell(&EnableCloneCellPayload {
            app_id: app_id.clone(),
            clone_cell_id: CloneCellId::CellId(clone_cell_2.cell_id.clone()),
        })
        .await
        .unwrap();
    assert_eq!(
        reenabled_clone_cell.original_dna_hash,
        dna_2.dna_hash().clone()
    );
}
