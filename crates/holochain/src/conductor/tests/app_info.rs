use holochain_conductor_api::CellInfo;
use holochain_types::prelude::{CreateCloneCellPayload, InstalledAppId};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{DnaModifiersOpt, RoleName};
use matches::matches;

use crate::sweettest::{SweetAgents, SweetConductor, SweetDnaFile};

#[tokio::test(flavor = "multi_thread")]
async fn app_info_returns_dna_modifiers() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let agent_pub_key = SweetAgents::one(conductor.keystore()).await;
    let app_id: InstalledAppId = "app".into();
    let role_name: RoleName = "dna".into();
    conductor
        .setup_app_for_agent(
            &app_id,
            agent_pub_key.clone(),
            [&(role_name.clone(), dna.clone())],
        )
        .await
        .unwrap();

    let installed_clone_cell = conductor
        .clone()
        .create_clone_cell(CreateCloneCellPayload {
            app_id: app_id.clone(),
            role_name: role_name.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("seed".to_string()),
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();

    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    // app info has cell info for one role name
    assert_eq!(app_info.cell_info.len(), 1);
    println!("app info {:?}", app_info);

    let cell_info_for_role = app_info.cell_info.get(&role_name).unwrap();
    // first cell in cell info is provisioned cell
    matches!(cell_info_for_role[0], CellInfo::Provisioned(_));
    // second cell in cell info is clone cell
    matches!(cell_info_for_role[1], CellInfo::Cloned(_));

    // clone cell id matches
    assert!(if let CellInfo::Cloned(cell) = &cell_info_for_role[1] {
        cell.cell_id == installed_clone_cell.as_id().clone()
    } else {
        false
    });
}
