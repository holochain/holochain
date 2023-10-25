use holochain::sweettest::SweetAgents;
use holochain::sweettest::SweetConductor;
use holochain_keystore::MetaLairClient;
use holochain_p2p::dht::prelude::Topology;
use holochain_p2p::dht::PeerStrat;
use holochain_p2p::dht_arc::DEFAULT_MIN_PEERS;
use holochain_p2p::dht_arc::DEFAULT_MIN_REDUNDANCY;
use holochain_p2p::dht_arc::MAX_HALF_LENGTH;
use kitsune_p2p::dht_arc::DhtArc;
use kitsune_p2p::*;
use kitsune_p2p_types::dht_arc::check_redundancy;

pub struct MyProperties {
    authority_agent: Vec<u8>,
    max_count: u32,
    contract_address: String
}

#[tokio::test(flavor = "multi_thread")]
// Can specify dna properties and then read those properties via the #[dna_properties] helper macro
async fn test_dna_properties_macro() {
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::DnaProperties])
        .await
        .unwrap();

    // Set dna properties
    let properties = MyProperties {
        authority_agent: [0u8; 36].to_vec(),
        max_count: 500,
        contract_address: "0x12345"
    };
    let properties_sb = SerializedBytes::try_from(properties).unwrap();
    let dnas = &[dna_file.update_modifiers(DnaModifiersOpt {
        network_seed: None,
        properties: Some(properties_sb),
        origin_time: None,
        quantum_time: None,
    })];
    
    // Create a Conductor
    let mut conductor = SweetConductor::from_config(Default::default()).await;

    let agents = SweetAgents::get(conductor.keystore(), 1).await;
    let apps = conductor
        .setup_app_for_agent("app", &agents, &[dna_file])
        .await
        .unwrap();
    let cells = apps.cells_flattened();
    let alice = cells[0].zome(TestWasm::DnaProperties);

    // Get dna properties via helper macro
    let received_properties: MyProperties = conductor.call(&alice, "get_dna_properties", ()).await;

    assert_eq!(received_properties, properties)    
}


pub struct MyInvalidProperties {
    bad_property: u32
}

#[tokio::test(flavor = "multi_thread")]
// Can specify dna properties and then read those properties via the #[dna_properties] helper macro
async fn test_dna_properties_fails_with_invalid_properties() {
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::DnaProperties])
        .await
        .unwrap();

    // Set dna properties
    let properties = MyInvalidProperties {
        bad_property: 500
    };
    let properties_sb = SerializedBytes::try_from(properties).unwrap();
    let dnas = &[dna_file.update_modifiers(DnaModifiersOpt {
        network_seed: None,
        properties: Some(properties_sb),
        origin_time: None,
        quantum_time: None,
    })];
    
    // Create a Conductor
    let mut conductor = SweetConductor::from_config(Default::default()).await;

    let agents = SweetAgents::get(conductor.keystore(), 1).await;
    let apps = conductor
        .setup_app_for_agent("app", &agents, &[dna_file])
        .await
        .unwrap();
    let cells = apps.cells_flattened();
    let alice = cells[0].zome(TestWasm::DnaProperties);

    // Try to get dna properties via helper macro
    let res = conductor.call_fallible(&alice, "get_dna_properties", ()).await;

    assert!(res.is_err())
}
