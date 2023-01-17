use holochain_conductor_api::CellInfo;
use holochain_types::prelude::{
    CloneCellId, CreateCloneCellPayload, DisableCloneCellPayload, InstalledAppId,
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

    let clone_name_1 = "clone_1".to_string();
    let installed_clone_cell_1 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app_id.clone(),
            role_name: role_name_1.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
            membrane_proof: None,
            name: Some(clone_name_1.clone()),
        })
        .await
        .unwrap();

    let clone_name_2 = "clone_2".to_string();
    let installed_clone_cell_2 = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app_id.clone(),
            role_name: role_name_2.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
            membrane_proof: None,
            name: Some(clone_name_2.clone()),
        })
        .await
        .unwrap();

    // disable clone cell 2
    conductor
        .disable_clone_cell(&DisableCloneCellPayload {
            app_id: app_id.clone(),
            clone_cell_id: CloneCellId::CellId(installed_clone_cell_2.cell_id.clone()),
        })
        .await
        .unwrap();

    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();

    // app info has cell info for two role names
    assert_eq!(app_info.cell_info.len(), 2);

    // check cell info for role name 1
    let cell_info_for_role_1 = app_info.cell_info.get(&role_name_1).unwrap();
    // first cell in cell info is provisioned cell
    matches!(cell_info_for_role_1[0], CellInfo::Provisioned(_));
    // second cell in cell info is clone cell
    matches!(cell_info_for_role_1[1], CellInfo::Cloned(_));

    // check cell info for role name 2
    let cell_info_for_role_2 = app_info.cell_info.get(&role_name_2).unwrap();
    // first cell in cell info is provisioned cell
    matches!(cell_info_for_role_2[0], CellInfo::Provisioned(_));
    // second cell in cell info is clone cell
    matches!(cell_info_for_role_2[1], CellInfo::Cloned(_));

    // clone cell ids match
    assert!(if let CellInfo::Cloned(cell) = &cell_info_for_role_1[1] {
        cell.cell_id == installed_clone_cell_1.cell_id.clone()
    } else {
        false
    });
    assert!(if let CellInfo::Cloned(cell) = &cell_info_for_role_2[1] {
        cell.cell_id == installed_clone_cell_2.cell_id.clone()
    } else {
        false
    });

    conductor.shutdown().await;
    conductor.startup().await;

    // make sure app info is identical after conductor restart
    let app_info_after_restart = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    // println!("app info before {:#?}\nand after restart {:#?}", app_info, app_info_after_restart);
    assert_eq!(app_info, app_info_after_restart);
}
