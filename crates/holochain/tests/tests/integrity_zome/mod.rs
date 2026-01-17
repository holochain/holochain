use holo_hash::ActionHash;
use holo_hash::WasmHash;
use holochain::conductor::api::AdminInterfaceApi;
use holochain::sweettest::*;
use holochain_conductor_api::AdminRequest;
use holochain_conductor_api::AdminResponse;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestCoordinatorWasm;
use holochain_wasm_test_utils::TestIntegrityWasm;
use holochain_wasm_test_utils::TestWasm;
use mr_bundle::Bundle;
use serde::Serialize;
use std::path::PathBuf;

#[tokio::test(flavor = "multi_thread")]
async fn test_coordinator_zome_update() {
    let mut conductor = SweetConductor::standard().await;
    let (dna, _, _) = SweetDnaFile::unique_from_zomes(
        vec![TestIntegrityWasm::IntegrityZome],
        vec![TestCoordinatorWasm::CoordinatorZome],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZome),
        ],
    )
    .await;

    println!("Install Dna with integrity and coordinator zomes.");
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cells = app.into_cells();

    println!("Create an entry.");
    let hash: ActionHash = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "create_entry",
            (),
        )
        .await;

    println!("Try getting the entry from the coordinator zome.");
    let record: Option<Record> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "get_entry",
            (),
        )
        .await;

    assert!(record.is_some());

    // Update the coordinator zomes for a totally different coordinator zome (conductor is still running)
    conductor
        .update_coordinators(
            cells[0].cell_id().clone(),
            vec![CoordinatorZome::from(TestCoordinatorWasm::CoordinatorZomeUpdate).into_inner()],
            vec![TestCoordinatorWasm::CoordinatorZomeUpdate.into()],
        )
        .await
        .unwrap();

    // Try getting the entry from the new coordinator zome
    let record: Option<Record> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash.clone(),
        )
        .await;

    assert!(record.is_some());

    // Now restart the conductor with 'ignore_dna_files_cache = true' and try
    // calling the updated coordinator zome again to verify that the coordinator
    // zome update is persisted correctly in the database and not only in the
    // in-memory ribosome store
    conductor.shutdown().await;
    conductor.startup(true).await;

    let record: Option<Record> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash,
        )
        .await;

    assert!(record.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_coordinator_zome_update_multi_integrity() {
    let mut conductor = SweetConductor::standard().await;
    let mut second_integrity = IntegrityZome::from(TestIntegrityWasm::IntegrityZome);
    second_integrity.zome_name_mut().0 = "2".into();
    let (_, second_coordinator) =
        CoordinatorZome::from(TestCoordinatorWasm::CoordinatorZome).into_inner();

    let second_coordinator = match second_coordinator.erase_type() {
        ZomeDef::Wasm(WasmZome {
            wasm_hash,
            mut dependencies,
        }) => {
            dependencies.clear();
            dependencies.push("2".into());

            Zome::<CoordinatorZomeDef>::new(
                "2_coord".into(),
                ZomeDef::Wasm(WasmZome {
                    wasm_hash,
                    dependencies,
                })
                .into(),
            )
        }
        _ => todo!(),
    };

    let (dna, _, _) = SweetDnaFile::unique_from_zomes(
        vec![
            IntegrityZome::from(TestIntegrityWasm::IntegrityZome),
            second_integrity,
        ],
        vec![
            CoordinatorZome::from(TestCoordinatorWasm::CoordinatorZome),
            second_coordinator,
        ],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZome),
        ],
    )
    .await;

    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cells = app.into_cells();

    let hash: ActionHash = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "create_entry",
            (),
        )
        .await;

    let hash2: ActionHash = conductor
        .call(&cells[0].zome("2_coord"), "create_entry", ())
        .await;

    let record: Option<Record> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "get_entry",
            (),
        )
        .await;

    assert!(record.is_some());
    let record: Option<Record> = conductor
        .call(&cells[0].zome("2_coord"), "get_entry", ())
        .await;

    assert!(record.is_some());

    // Add a completely new coordinator with the same dependency
    conductor
        .update_coordinators(
            cells[0].cell_id().clone(),
            vec![CoordinatorZome::from(TestCoordinatorWasm::CoordinatorZomeUpdate).into_inner()],
            vec![TestCoordinatorWasm::CoordinatorZomeUpdate.into()],
        )
        .await
        .unwrap();

    let record: Option<Record> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash,
        )
        .await;

    assert!(record.is_some());

    // Replace "2_coord" with different zome but same dependecies.
    let wasm_hash =
        WasmHash::with_data(&DnaWasm::from(TestCoordinatorWasm::CoordinatorZomeUpdate)).await;
    let new_coordinator: CoordinatorZomeDef = ZomeDef::Wasm(WasmZome {
        wasm_hash,
        dependencies: vec!["2".into()],
    })
    .into();

    conductor
        .update_coordinators(
            cells[0].cell_id().clone(),
            vec![("2_coord".into(), new_coordinator)],
            vec![TestCoordinatorWasm::CoordinatorZomeUpdate.into()],
        )
        .await
        .unwrap();

    let record: Option<Record> = conductor
        .call(&cells[0].zome("2_coord"), "get_entry", hash2.clone())
        .await;

    assert!(record.is_some());

    // Now restart the conductor with 'ignore_dna_files_cache = true' and try
    // calling the updated coordinator zome again to verify that the coordinator
    // zome update is persisted correctly in the database and not only in the
    // in-memory ribosome store
    conductor.shutdown().await;
    conductor.startup(true).await;

    let record: Option<Record> = conductor
        .call(&cells[0].zome("2_coord"), "get_entry", hash2)
        .await;

    assert!(record.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_admin_interface() {
    let mut conductor = SweetConductor::standard().await;
    let (dna, _, _) = SweetDnaFile::unique_from_zomes(
        vec![TestIntegrityWasm::IntegrityZome],
        vec![TestCoordinatorWasm::CoordinatorZome],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZome),
        ],
    )
    .await;

    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cells = app.into_cells();

    let hash: ActionHash = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "create_entry",
            (),
        )
        .await;

    let admin_api = AdminInterfaceApi::new(conductor.clone());

    let path: PathBuf = TestCoordinatorWasm::CoordinatorZomeUpdate.into();
    let manifest = CoordinatorManifest {
        zomes: vec![ZomeManifest {
            name: TestCoordinatorWasm::CoordinatorZomeUpdate.into(),
            hash: None,
            path: path.display().to_string(),
            dependencies: Some(vec![ZomeDependency {
                name: TestIntegrityWasm::IntegrityZome.into(),
            }]),
        }],
    };

    let code = DnaWasm::from(TestCoordinatorWasm::CoordinatorZomeUpdate)
        .code
        .to_vec()
        .into();

    let source: CoordinatorBundle = Bundle::new(
        manifest,
        [(
            path.file_name().unwrap().to_str().unwrap().to_string(),
            code,
        )],
    )
    .unwrap()
    .into();

    println!("Bundle: {source:?}");

    let req = UpdateCoordinatorsPayload {
        cell_id: cells[0].cell_id().clone(),
        source: CoordinatorSource::Bundle(Box::new(source)),
    };
    let req = AdminRequest::UpdateCoordinators(Box::new(req));
    let r = admin_api.handle_request(Ok(req)).await.unwrap();
    assert!(matches!(r, AdminResponse::CoordinatorsUpdated));

    let record: Option<Record> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash,
        )
        .await;

    assert!(record.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_wasm_memory() {
    let mut conductor = SweetConductor::standard().await;
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cells = app.into_cells();

    #[derive(Debug, Serialize)]
    struct Post(String);

    let data = String::from_utf8(vec![0u8; 3_000_000]).unwrap();

    let mut cum = 0;
    for i in 0..100 {
        cum += data.len();
        eprintln!("committing {} {} {:?}", i, cum, Timestamp::now());
        let _hash: ActionHash = conductor
            .call(
                &cells[0].zome(TestWasm::Create),
                "create_post",
                Post(data.clone()),
            )
            .await;
    }
}
