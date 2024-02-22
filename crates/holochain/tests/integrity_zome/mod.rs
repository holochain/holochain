use std::path::PathBuf;

use holo_hash::ActionHash;
use holo_hash::WasmHash;
use holochain::conductor::api::AdminInterfaceApi;
use holochain::conductor::api::RealAdminInterfaceApi;
use holochain::sweettest::*;
use holochain_conductor_api::AdminRequest;
use holochain_conductor_api::AdminResponse;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestCoordinatorWasm;
use holochain_wasm_test_utils::TestIntegrityWasm;
use holochain_wasm_test_utils::TestWasm;
use mr_bundle::Bundle;
use serde::Serialize;

#[tokio::test(flavor = "multi_thread")]
async fn test_coordinator_zome_update() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna, _, _) = SweetDnaFile::unique_from_zomes(
        vec![TestIntegrityWasm::IntegrityZome],
        vec![TestCoordinatorWasm::CoordinatorZome],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZome),
        ],
    )
    .await;
    let dna_hash = dna.dna_hash().clone();

    println!("Install Dna with integrity and coordinator zomes.");
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cells = app.into_cells();

    println!("Create entry from the coordinator zome into the integrity zome.");
    let hash: ActionHash = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "create_entry",
            (),
        )
        .await;
    println!("Success!");

    println!("Try getting the entry from the coordinator zome.");
    let record: Option<Record> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "get_entry",
            (),
        )
        .await;

    assert!(record.is_some());
    println!("Success!");

    println!("Update the coordinator zomes for a totally different coordinator zome (conductor is still running)");
    conductor
        .update_coordinators(
            &dna_hash,
            vec![CoordinatorZome::from(TestCoordinatorWasm::CoordinatorZomeUpdate).into_inner()],
            vec![TestCoordinatorWasm::CoordinatorZomeUpdate.into()],
        )
        .await
        .unwrap();
    println!("Success!");

    println!("Try getting the entry from the new coordinator zome.");
    let record: Option<Record> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash,
        )
        .await;

    assert!(record.is_some());
    println!("Success! Success! Success! ");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_coordinator_zome_update_multi_integrity() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let mut second_integrity = IntegrityZome::from(TestIntegrityWasm::IntegrityZome);
    second_integrity.zome_name_mut().0 = "2".into();
    let (_, second_coordinator) =
        CoordinatorZome::from(TestCoordinatorWasm::CoordinatorZome).into_inner();

    let second_coordinator = match second_coordinator.erase_type() {
        ZomeDef::Wasm(WasmZome {
            wasm_hash,
            mut dependencies,
            preserialized_path,
        }) => {
            dependencies.clear();
            dependencies.push("2".into());

            Zome::<CoordinatorZomeDef>::new(
                "2_coord".into(),
                ZomeDef::Wasm(WasmZome {
                    wasm_hash,
                    dependencies,
                    preserialized_path,
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

    let dna_hash = dna.dna_hash().clone();

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
            &dna_hash,
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
        preserialized_path: None,
    })
    .into();

    conductor
        .update_coordinators(
            &dna_hash,
            vec![("2_coord".into(), new_coordinator)],
            vec![TestCoordinatorWasm::CoordinatorZomeUpdate.into()],
        )
        .await
        .unwrap();

    let record: Option<Record> = conductor
        .call(&cells[0].zome("2_coord"), "get_entry", hash2)
        .await;

    assert!(record.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_admin_interface() {
    let mut conductor = SweetConductor::from_standard_config().await;
    let (dna, _, _) = SweetDnaFile::unique_from_zomes(
        vec![TestIntegrityWasm::IntegrityZome],
        vec![TestCoordinatorWasm::CoordinatorZome],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZome),
        ],
    )
    .await;

    let dna_hash = dna.dna_hash().clone();

    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cells = app.into_cells();

    let hash: ActionHash = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "create_entry",
            (),
        )
        .await;

    let admin_api = RealAdminInterfaceApi::new(conductor.clone());

    let manifest = CoordinatorManifest {
        zomes: vec![ZomeManifest {
            name: TestCoordinatorWasm::CoordinatorZomeUpdate.into(),
            hash: None,
            dylib: None,
            location: ZomeLocation::Bundled(TestCoordinatorWasm::CoordinatorZomeUpdate.into()),
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
            PathBuf::from(TestCoordinatorWasm::CoordinatorZomeUpdate),
            code,
        )],
        env!("CARGO_MANIFEST_DIR").into(),
    )
    .unwrap()
    .into();

    let req = UpdateCoordinatorsPayload {
        dna_hash,
        source: holochain_types::prelude::CoordinatorSource::Bundle(Box::new(source)),
    };
    let req = AdminRequest::UpdateCoordinators(Box::new(req));
    let r = admin_api.handle_admin_request(req).await;
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
    let mut conductor = SweetConductor::from_standard_config().await;
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
