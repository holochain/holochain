use crate::conductor::conductor::CellStatus;
use crate::sweettest::{SweetConductor, SweetDnaFile};
use hdk::prelude::{CellId, DnaModifiersOpt, NetworkSeed, RoleName};
use holochain_types::app::{
    AppBundle, AppBundleSource, AppManifest, AppManifestCurrentBuilder, AppRoleAssignment,
    AppRoleDnaManifest, AppRoleManifest, AppRolePrimary, AppStatus, CellProvisioning,
    DisabledAppReason, InstallAppPayload, InstalledApp, InstalledAppCommon, InstalledAppId,
    InstalledAppMap,
};
use holochain_types::dna::{DnaBundle, DnaFile};
use holochain_wasm_test_utils::TestWasm;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread")]
async fn app_operations() {
    let conductor = SweetConductor::from_standard_config().await;

    // Check conductor state is empty.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    assert_eq!(*state.installed_apps(), InstalledAppMap::new());

    // Running cells should be empty.
    conductor.running_cells.share_ref(|cells| {
        assert!(cells.is_empty());
    });

    // Install an app with 1 DNA.
    let app_id_1: InstalledAppId = "app_1".to_string();
    let role_1: RoleName = "role".to_string();
    let clone_limit_1 = 10;
    let App {
        bundle: bundle_1,
        dna_files: dna_files_1,
        manifest: manifest_1,
    } = make_app(
        app_id_1.clone(),
        app_id_1.clone(),
        [(vec![TestWasm::AgentInfo], role_1.clone())],
        clone_limit_1,
    )
    .await;
    let app_1 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bytes(bundle_1.pack().unwrap()),
            agent_key: None,
            installed_app_id: Some(app_id_1.clone()),
            network_seed: None,
            roles_settings: None,
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();

    // Check conductor state only contains the installed app.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    let mut expected_app_map = InstalledAppMap::new();
    expected_app_map.insert(
        app_id_1.clone(),
        InstalledApp::new(
            InstalledAppCommon::new(
                app_id_1.clone(),
                app_1.agent_key.clone(),
                [(
                    role_1.clone(),
                    AppRoleAssignment::Primary(AppRolePrimary::new(
                        dna_files_1[0].dna_hash().clone(),
                        true,
                        clone_limit_1,
                    )),
                )],
                manifest_1.clone(),
                app_1.installed_at,
            )
            .unwrap(),
            AppStatus::Disabled(DisabledAppReason::NeverStarted),
        ),
    );
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Enable app 1 now
    conductor.enable_app(app_id_1.clone()).await.unwrap();

    // Conductor state should reflect that the app is enabled.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    expected_app_map.get_mut(&app_id_1).unwrap().status = AppStatus::Enabled;
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should only contain the installed cell.
    conductor.running_cells.share_ref(|cells| {
        assert_eq!(cells.len(), 1);
        let (cell_id, cell_item) = cells.get_index(0).unwrap();
        assert_eq!(
            *cell_id,
            CellId::new(dna_files_1[0].dna_hash().clone(), app_1.agent_key.clone())
        );
        assert_eq!(cell_item.status, CellStatus::Joined);
        assert_eq!(cell_id, cell_item.cell.id());
    });

    // Disable app
    conductor
        .disable_app(app_id_1.clone(), DisabledAppReason::User)
        .await
        .unwrap();

    // Conductor state should reflect that the app is disabled.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    expected_app_map.get_mut(&app_id_1).unwrap().status =
        AppStatus::Disabled(DisabledAppReason::User);
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should again be empty.
    conductor.running_cells.share_ref(|cells| {
        assert!(cells.is_empty());
    });

    // Enable app again
    conductor.enable_app(app_id_1.clone()).await.unwrap();

    // Conductor state should reflect that the app is enabled.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    expected_app_map.get_mut(&app_id_1).unwrap().status = AppStatus::Enabled;
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should contain only the installed cell.
    conductor.running_cells.share_ref(|cells| {
        assert_eq!(cells.len(), 1);
        let (cell_id, cell_item) = cells.get_index(0).unwrap();
        assert_eq!(
            *cell_id,
            CellId::new(dna_files_1[0].dna_hash().clone(), app_1.agent_key.clone())
        );
        assert_eq!(cell_id, cell_item.cell.id());
        assert_eq!(cell_item.status, CellStatus::Joined);
    });

    // Install another app with 2 DNAs
    let app_id_2: InstalledAppId = "app_2".to_string();
    let role_2_1: RoleName = "role_2_1".to_string();
    let role_2_2: RoleName = "role_2_2".to_string();
    let clone_limit_2 = 10;
    let App {
        bundle: bundle_2,
        dna_files: dna_files_2,
        manifest: manifest_2,
    } = make_app(
        app_id_2.clone(),
        app_id_2.clone(),
        [
            (vec![TestWasm::Foo], role_2_1.clone()),
            (vec![TestWasm::InitPass], role_2_2.clone()),
        ],
        clone_limit_2,
    )
    .await;
    let app_2 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bytes(bundle_2.pack().unwrap()),
            agent_key: None,
            installed_app_id: Some(app_id_2.clone()),
            network_seed: None,
            roles_settings: None,
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();

    // Enable app 2
    conductor.enable_app(app_id_2.clone()).await.unwrap();

    // Check conductor state contains both installed apps.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    expected_app_map.insert(
        app_id_2.clone(),
        InstalledApp::new(
            InstalledAppCommon::new(
                app_id_2.clone(),
                app_2.agent_key.clone(),
                [
                    (
                        role_2_1.clone(),
                        AppRoleAssignment::Primary(AppRolePrimary::new(
                            dna_files_2[0].dna_hash().clone(),
                            true,
                            clone_limit_2,
                        )),
                    ),
                    (
                        role_2_2.clone(),
                        AppRoleAssignment::Primary(AppRolePrimary::new(
                            dna_files_2[1].dna_hash().clone(),
                            true,
                            clone_limit_2,
                        )),
                    ),
                ],
                manifest_2.clone(),
                app_2.installed_at,
            )
            .unwrap(),
            AppStatus::Enabled,
        ),
    );
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should contain cells of both apps.
    conductor.running_cells.share_ref(|cells| {
        assert_eq!(cells.len(), 3);
        // Cell of app 1
        let expected_cell_id =
            CellId::new(dna_files_1[0].dna_hash().clone(), app_1.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
        assert_eq!(cell_item.status, CellStatus::Joined);
        // Cell 1 of app 2
        let expected_cell_id =
            CellId::new(dna_files_2[0].dna_hash().clone(), app_2.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
        assert_eq!(cell_item.status, CellStatus::Joined);
        // Cell 2 of app 2
        let expected_cell_id =
            CellId::new(dna_files_2[1].dna_hash().clone(), app_2.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
        assert_eq!(cell_item.status, CellStatus::Joined);
    });

    // Uninstall app 1
    conductor
        .clone()
        .uninstall_app(&app_id_1, false)
        .await
        .unwrap();

    // Conductor state should reflect the app is gone.
    let state = conductor.get_state().await.unwrap();
    expected_app_map.shift_remove(&app_id_1);
    assert_eq!(state.app_interfaces, HashMap::new());
    assert_eq!(*state.installed_apps(), expected_app_map);

    // Running cells should only contain only the cells of app 2.
    conductor.running_cells.share_ref(|cells| {
        // Cell 1 of app 2
        let expected_cell_id =
            CellId::new(dna_files_2[0].dna_hash().clone(), app_2.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
        assert_eq!(cell_item.status, CellStatus::Joined);
        // Cell 2 of app 2
        let expected_cell_id =
            CellId::new(dna_files_2[1].dna_hash().clone(), app_2.agent_key.clone());
        let cell_item = cells.get(&expected_cell_id).unwrap();
        assert_eq!(*cell_item.cell.id(), expected_cell_id);
        assert_eq!(cell_item.status, CellStatus::Joined);
    });

    // Uninstall app 2
    conductor
        .clone()
        .uninstall_app(&app_id_2, false)
        .await
        .unwrap();

    // Conductor state should reflect no more apps are installed.
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state.app_interfaces, HashMap::new());
    assert_eq!(*state.installed_apps(), InstalledAppMap::new());

    // Running cells should be empty.
    conductor.running_cells.share_ref(|cells| {
        assert!(cells.is_empty());
    });
}

struct App {
    bundle: AppBundle,
    dna_files: Vec<DnaFile>,
    manifest: AppManifest,
}

async fn make_app(
    app_id: InstalledAppId,
    network_seed: NetworkSeed,
    test_wasms_with_roles: impl IntoIterator<Item = (Vec<TestWasm>, RoleName)>,
    clone_limit: u32,
) -> App {
    let modifiers = DnaModifiersOpt::none().with_network_seed(network_seed.clone());
    let mut dna_files = Vec::new();
    let mut app_role_manifests = Vec::new();
    let mut resources = Vec::new();
    for (test_wasms, role) in test_wasms_with_roles {
        let dna_file =
            SweetDnaFile::from_test_wasms(network_seed.clone(), test_wasms, Default::default())
                .await
                .0;
        let dna_path = format!("{}", dna_file.dna_hash());
        let app_role_manifest = AppRoleManifest {
            name: role,
            dna: AppRoleDnaManifest {
                path: Some(dna_path.clone()),
                modifiers: modifiers.clone(),
                installed_hash: Some(dna_file.dna_hash().clone().into()),
                clone_limit,
            },
            provisioning: Some(CellProvisioning::Create { deferred: false }),
        };
        app_role_manifests.push(app_role_manifest);
        let dna_bundle = DnaBundle::from_dna_file(dna_file.clone()).unwrap();
        resources.push((dna_path, dna_bundle));
        dna_files.push(dna_file);
    }
    let manifest: AppManifest = AppManifestCurrentBuilder::default()
        .name(app_id)
        .description(None)
        .roles(app_role_manifests)
        .allow_deferred_memproofs(false)
        .build()
        .unwrap()
        .into();
    let bundle = AppBundle::new(manifest.clone(), resources).unwrap();

    App {
        bundle,
        dna_files,
        manifest,
    }
}
