use hdk::prelude::*;
use holochain::sweettest::SweetAgents;
use holochain::sweettest::SweetConductor;
use holochain::sweettest::SweetDnaFile;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_test_wasm_common::MyValidDnaProperties;
use holochain_types::prelude::DnaModifiersOpt;
use holochain_wasm_test_utils::TestWasm;
use serde::{Deserialize, Serialize};

#[tokio::test(flavor = "multi_thread")]
// Can specify dna properties and then read those properties via the #[dna_properties] helper macro
async fn test_dna_properties_macro() {
    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::DnaProperties]).await;

    // Set DNA Properties
    let properties = MyValidDnaProperties {
        authority_agent: [0u8; 36].to_vec(),
        max_count: 500,
        contract_address: String::from("0x12345"),
    };
    let properties_sb: SerializedBytes = properties.clone().try_into().unwrap();
    let dnas = &[dna_file.update_modifiers(DnaModifiersOpt {
        network_seed: None,
        properties: Some(properties_sb),
        origin_time: None,
        quantum_time: None,
    })];

    // Create a Conductor
    let mut conductor = SweetConductor::from_config(ConductorConfig::default()).await;
    let app = conductor.setup_app("app", dnas).await.unwrap();
    let alice_zome = app.cells()[0].zome(TestWasm::DnaProperties);

    // Get DNA Properties via helper macro
    let received_properties: MyValidDnaProperties =
        conductor.call(&alice_zome, "get_dna_properties", ()).await;

    assert_eq!(received_properties, properties)
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, SerializedBytes)]
pub struct MyInvalidProperties {
    bad_property: u32,
}

#[tokio::test(flavor = "multi_thread")]
// Can specify dna properties and then read those properties via the #[dna_properties] helper macro
async fn test_dna_properties_fails_with_invalid_properties() {
    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::DnaProperties]).await;

    // Set DNA Properties
    let properties = MyInvalidProperties { bad_property: 500 };
    let properties_sb: SerializedBytes = properties.try_into().unwrap();
    let modifiers = DnaModifiersOpt {
        network_seed: None,
        properties: Some(properties_sb),
        origin_time: None,
        quantum_time: None,
    };
    let dnas = &[dna_file.update_modifiers(modifiers)];

    // Create a Conductor
    let mut conductor = SweetConductor::from_config(ConductorConfig::default()).await;
    let app = conductor.setup_app("app", dnas).await.unwrap();
    let alice_zome = app.cells()[0].zome(TestWasm::DnaProperties);

    // Fail to get DNA Properties via helper macro
    let res: Result<MyValidDnaProperties, _> = conductor
        .call_fallible(&alice_zome, "get_dna_properties", ())
        .await;

    assert!(res.is_err())
}
