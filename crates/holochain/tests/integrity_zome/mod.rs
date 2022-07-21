use std::path::PathBuf;

use holo_hash::ActionHash;
use holo_hash::WasmHash;
use holochain::conductor::api::AdminInterfaceApi;
use holochain::conductor::api::RealAdminInterfaceApi;
use holochain::sweettest::*;
use holochain_conductor_api::AdminRequest;
use holochain_conductor_api::AdminResponse;
use holochain_types::dna::CoordinatorBundle;
use holochain_types::dna::CoordinatorManifest;
use holochain_types::dna::ZomeDependency;
use holochain_types::dna::ZomeLocation;
use holochain_types::dna::ZomeManifest;
use holochain_types::prelude::DnaWasm;
use holochain_types::prelude::HotSwapCoordinatorsPayload;
use holochain_wasm_test_utils::TestCoordinatorWasm;
use holochain_wasm_test_utils::TestIntegrityWasm;
use holochain_zome_types::CoordinatorZome;
use holochain_zome_types::CoordinatorZomeDef;
use holochain_zome_types::IntegrityZome;
use holochain_zome_types::Record;
use holochain_zome_types::WasmZome;
use holochain_zome_types::Zome;
use holochain_zome_types::ZomeDef;
use mr_bundle::Bundle;

#[tokio::test(flavor = "multi_thread")]
async fn test_coordinator_zome_hot_swap() {
    let mut conductor = SweetConductor::from_config(Default::default()).await;
    let (dna, _, _) = SweetDnaFile::unique_from_zomes(
        vec![TestIntegrityWasm::IntegrityZome],
        vec![TestCoordinatorWasm::CoordinatorZome],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZome),
        ],
    )
    .await
    .unwrap();
    let dna_hash = dna.dna_hash().clone();

    println!("Install Dna with integrity and coordinator zomes.");
    let app = conductor.setup_app("app", &[dna]).await.unwrap();
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

    println!("Hot swap the coordinator zomes for a totally different coordinator zome (conductor is still running)");
    conductor
        .hot_swap_coordinators(
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
async fn test_coordinator_zome_hot_swap_multi_integrity() {
    let mut conductor = SweetConductor::from_config(Default::default()).await;
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
    .await
    .unwrap();

    let dna_hash = dna.dna_hash().clone();

    let app = conductor.setup_app("app", &[dna]).await.unwrap();
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
        .hot_swap_coordinators(
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
    })
    .into();

    conductor
        .hot_swap_coordinators(
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
async fn test_hot_swap_admin_interface() {
    let mut conductor = SweetConductor::from_config(Default::default()).await;
    let (dna, _, _) = SweetDnaFile::unique_from_zomes(
        vec![TestIntegrityWasm::IntegrityZome],
        vec![TestCoordinatorWasm::CoordinatorZome],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZome),
        ],
    )
    .await
    .unwrap();

    let dna_hash = dna.dna_hash().clone();

    let _app = conductor.setup_app("app", &[dna]).await.unwrap();

    let admin_api = RealAdminInterfaceApi::new(conductor.clone());

    let manifest = CoordinatorManifest {
        zomes: vec![ZomeManifest {
            name: TestCoordinatorWasm::CoordinatorZomeUpdate.into(),
            hash: None,
            location: ZomeLocation::Bundled(TestCoordinatorWasm::CoordinatorZomeUpdate.into()),
            dependencies: Some(vec![ZomeDependency {
                name: TestIntegrityWasm::IntegrityZome.into(),
            }]),
        }],
    };

    let code = DnaWasm::from(TestCoordinatorWasm::CoordinatorZomeUpdate)
        .code
        .to_vec();

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

    let req = HotSwapCoordinatorsPayload {
        dna_hash,
        source: holochain_types::prelude::CoordinatorSource::Bundle(Box::new(source)),
    };
    let req = AdminRequest::HotSwapCoordinators(Box::new(req));
    let r = admin_api.handle_admin_request(req).await;
    assert!(matches!(r, AdminResponse::CoordinatorsHotSwapped));
}
