use hdk::prelude::{CoordinatorZome, IntegrityZome, Record, SerializedBytes};
use holo_hash::ActionHash;
use holochain::{
    prelude::DnaWasm,
    sweettest::{SweetConductor, SweetDnaFile},
};
use holochain_wasm_test_utils::{TestCoordinatorWasm, TestIntegrityWasm};

/// Regression test for https://github.com/holochain/holochain/issues/2145#issuecomment-2460304757
#[tokio::test(flavor = "multi_thread")]
async fn can_install_two_apps_with_the_same_dna_hash_but_different_coordinators() {
    let dna_file_1 = SweetDnaFile::from_zomes(
        "the same seed".into(),
        vec![IntegrityZome::from(TestIntegrityWasm::IntegrityZome)],
        vec![CoordinatorZome::from(TestCoordinatorWasm::CoordinatorZome)],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZome),
        ],
        SerializedBytes::default(),
    )
    .await
    .0;

    // Create another DnaFile with a different coordinator zome
    let dna_file_2 = SweetDnaFile::from_zomes(
        "the same seed".into(),
        vec![IntegrityZome::from(TestIntegrityWasm::IntegrityZome)],
        vec![CoordinatorZome::from(
            TestCoordinatorWasm::CoordinatorZomeUpdate,
        )],
        vec![
            DnaWasm::from(TestIntegrityWasm::IntegrityZome),
            DnaWasm::from(TestCoordinatorWasm::CoordinatorZomeUpdate),
        ],
        SerializedBytes::default(),
    )
    .await
    .0;

    // Verify that the dna hashes match
    assert_eq!(dna_file_1.dna_hash(), dna_file_2.dna_hash());

    // Install the first app and write an entry to the DHT
    let mut conductor = SweetConductor::from_standard_config().await;

    let cells_1 = conductor
        .setup_app("app_1", [&dna_file_1])
        .await
        .unwrap()
        .into_cells();

    let hash: ActionHash = conductor
        .call(
            &cells_1[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "create_entry",
            (),
        )
        .await;

    // Now read the just created entry
    let record_1: Option<Record> = conductor
        .call(
            &cells_1[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "get_entry",
            (),
        )
        .await;
    assert!(record_1.is_some());

    // Install the second app and make a zome call to its coordinator zome, reading
    // the same entry from the shared DHT
    let cells_2 = conductor
        .setup_app("app_2", [&dna_file_2])
        .await
        .unwrap()
        .into_cells();

    let record_2: Option<Record> = conductor
        .call(
            &cells_2[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash.clone(),
        )
        .await;
    assert!(record_2.is_some());

    // Also make a zome call into the first app's coordinator zome again to ensure its
    // coodinator zome had not been overwritten when installing the second app
    let record_1: Option<Record> = conductor
        .call(
            &cells_1[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "get_entry",
            (),
        )
        .await;
    assert!(record_1.is_some());

    // Now restart the conductor *without* using the dna files cache of the
    // SweetConductor to ensure that the dna definitions with the appropriate
    // coordinator zomes had been persisted in the database and are loaded
    // correctly into the ribosome store on startup
    conductor.shutdown().await;
    conductor.startup(Some(true)).await; // important: ignore_dna_files_cache = true

    // Repeat the zome calls to verify that both coordinator zomes can be addressed
    // correctly
    let record_1: Option<Record> = conductor
        .call(
            &cells_1[0].zome(TestCoordinatorWasm::CoordinatorZome),
            "get_entry",
            (),
        )
        .await;
    assert!(record_1.is_some());

    let record_2: Option<Record> = conductor
        .call(
            &cells_2[0].zome(TestCoordinatorWasm::CoordinatorZomeUpdate),
            "get_entry",
            hash.clone(),
        )
        .await;
    assert!(record_2.is_some());
}
