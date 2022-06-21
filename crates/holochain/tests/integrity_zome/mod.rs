use holo_hash::ActionHash;
use holo_hash::WasmHash;
use holochain::sweettest::*;
use holochain_types::prelude::DnaWasm;
use holochain_wasm_test_utils::TestCoordinatorWasm;
use holochain_wasm_test_utils::TestIntegrityWasm;
use holochain_zome_types::Commit;
use holochain_zome_types::CoordinatorZome;
use holochain_zome_types::CoordinatorZomeDef;
use holochain_zome_types::IntegrityZome;
use holochain_zome_types::WasmZome;
use holochain_zome_types::Zome;
use holochain_zome_types::ZomeDef;

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
    let commit: Option<Commit> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "get_entry",
            (),
        )
        .await;

    assert!(commit.is_some());
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
    let commit: Option<Commit> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash,
        )
        .await;

    assert!(commit.is_some());
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

    let commit: Option<Commit> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "get_entry",
            (),
        )
        .await;

    assert!(commit.is_some());
    let commit: Option<Commit> = conductor
        .call(&cells[0].zome("2_coord"), "get_entry", ())
        .await;

    assert!(commit.is_some());

    // Add a completely new coordinator with the same dependency
    conductor
        .hot_swap_coordinators(
            &dna_hash,
            vec![CoordinatorZome::from(TestCoordinatorWasm::CoordinatorZomeUpdate).into_inner()],
            vec![TestCoordinatorWasm::CoordinatorZomeUpdate.into()],
        )
        .await
        .unwrap();

    let commit: Option<Commit> = conductor
        .call(
            &cells[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash,
        )
        .await;

    assert!(commit.is_some());

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

    let commit: Option<Commit> = conductor
        .call(&cells[0].zome("2_coord"), "get_entry", hash2)
        .await;

    assert!(commit.is_some());
}
