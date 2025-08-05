use crate::sweettest::{SweetConductor, SweetDnaFile};
use hdk::prelude::{CoordinatorZome, IntegrityZome};
use holochain_serialized_bytes::SerializedBytes;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_wasm_test_utils::{TestWasm, TestWasmPair};

#[tokio::test(flavor = "multi_thread")]
async fn ribosome_lookup_inline_zomes() {
    // Set up two DNAs with identical hashes and different coordinator zomes
    // that return different results for the same zome call.
    let expected_result_1 = "1";
    let integrity_zome = ("integrity", "".to_string(), vec![], 0);
    let coordinator_zome_name = "zome";
    let zome_set_1 = InlineZomeSet::new(
        [integrity_zome.clone()],
        [(coordinator_zome_name, "".to_string())],
        [],
    )
    .function(coordinator_zome_name, "echo", |_, ()| {
        Ok(expected_result_1.to_string())
    });
    let dna_1 = SweetDnaFile::from_inline_zomes("".to_string(), zome_set_1)
        .await
        .0;

    let expected_result_2 = "2";
    let zome_set_2 = InlineZomeSet::new(
        [integrity_zome],
        [(coordinator_zome_name, "".to_string())],
        [],
    )
    .function(coordinator_zome_name, "echo", |_, ()| {
        Ok(expected_result_2.to_string())
    });
    let dna_2 = SweetDnaFile::from_inline_zomes("".to_string(), zome_set_2)
        .await
        .0;

    assert_eq!(dna_1.dna_hash(), dna_2.dna_hash());

    // Create conductor, install both apps and make zome calls.
    let mut conductor = SweetConductor::from_standard_config().await;
    let app_1 = conductor.setup_app("1", [&dna_1.clone()]).await.unwrap();
    let result_1: String = conductor
        .call(&app_1.cells()[0].zome(coordinator_zome_name), "echo", ())
        .await;
    assert_eq!(result_1, expected_result_1);
    let app_2 = conductor.setup_app("2", [&dna_2.clone()]).await.unwrap();
    let result_2: String = conductor
        .call(&app_2.cells()[0].zome(coordinator_zome_name), "echo", ())
        .await;
    assert_eq!(result_2, expected_result_2);
}

#[tokio::test(flavor = "multi_thread")]
async fn ribosome_lookup_test_wasms() {
    // Set up two DNAs with identical hashes and different coordinator zomes
    // that return different results for the same zome call.
    let expected_result_1 = "1";
    let dna_1 = SweetDnaFile::from_test_wasms(
        "".to_string(),
        vec![TestWasm::RibosomeLookup1],
        SerializedBytes::default(),
    )
    .await
    .0;

    // It's not possible to create two identical DNAs with TestWasms,
    // because the zomes/wasms have to be named identically, and the build script
    // outputs both to the same directory. The second wasm would thus overwrite
    // the first.
    // Instead the first DNA is used as basis and its coordinator zomes are
    // replaced with the second DNA's.
    let expected_result_2 = "2";
    let dna_2 = SweetDnaFile::from_test_wasms(
        "".to_string(),
        vec![TestWasm::RibosomeLookup2],
        SerializedBytes::default(),
    )
    .await;
    let mut dna_2_wasm = dna_2
        .0
        .clone()
        .into_parts()
        .1
        .into_iter()
        .next_back()
        .unwrap()
        .1;
    let dna_2_coordinator_wasm = dna_2.2[0].def.clone();
    // Use DNA 1 as basis and replace the coordinator zome.
    let mut dna_2 = dna_1.clone();
    dna_2
        .update_coordinators(
            vec![(
                TestWasm::RibosomeLookup2.coordinator_zome_name(),
                dna_2_coordinator_wasm,
            )],
            vec![dna_2_wasm],
        )
        .await
        .unwrap();

    assert_eq!(dna_1.dna_hash(), dna_2.dna_hash());

    let mut conductor = SweetConductor::from_standard_config().await;
    let app_1 = conductor.setup_app("1", [&dna_1.clone()]).await.unwrap();
    let result_1: String = conductor
        .call(
            &app_1.cells()[0].zome(TestWasm::RibosomeLookup1.coordinator_zome_name()),
            "echo",
            (),
        )
        .await;
    assert_eq!(result_1, expected_result_1);
    let app_2 = conductor.setup_app("2", [&dna_2.clone()]).await.unwrap();
    let result_2: String = conductor
        .call(
            &app_2.cells()[0].zome(TestWasm::RibosomeLookup2.coordinator_zome_name()),
            "echo",
            (),
        )
        .await;
    assert_eq!(result_2, expected_result_2);
}
