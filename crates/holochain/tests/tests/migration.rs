//! These tests are being created to start tracking migration patterns for happs. The tests are
//! not necessarily representing best practise but more the current options that Holochain provides
//! for migrating from one app version to another.

use holo_hash::ActionHash;
use holochain::sweettest::{SweetAgents, SweetConductor, SweetConductorConfig, SweetDnaFile};
use holochain_serialized_bytes::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::{random_network_seed, YamlProperties};

#[tokio::test(flavor = "multi_thread")]
async fn migrate_dna_with_second_app_install() {
    holochain_trace::test_run();

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    struct TestProperties {
        pub prev_dna_hash: holo_hash::DnaHash,
    }

    // Matches the new definition of `MyType`
    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    struct MyType {
        value: String,
        amount: u32,
    }

    let config = SweetConductorConfig::rendezvous(false)
        .apply_shared_rendezvous()
        .await
        .no_dpki_mustfix();
    let mut conductor = SweetConductor::from_config(config).await;

    let alice = SweetAgents::one(conductor.keystore()).await;

    // Install the first version of the app
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::MigrateInitial]).await;

    let app_initial = conductor
        .setup_app_for_agent("app_initial", alice.clone(), &[dna.clone()])
        .await
        .unwrap();

    let alice_cells = app_initial.into_cells();
    let alice_cell = alice_cells.first().unwrap();

    // Create some data in the first version of the app
    let _: ActionHash = conductor
        .call(&alice_cell.zome(TestWasm::MigrateInitial), "create", ())
        .await;

    // Prepare the DNA for the new app version
    let mut mapping = serde_yaml::Mapping::new();
    mapping.insert(
        "prev_dna_hash".into(),
        serde_yaml::Value::String(dna.dna_hash().clone().to_string()),
    );
    let properties = YamlProperties::new(serde_yaml::Value::Mapping(mapping));
    let (new_dna, _, _) = SweetDnaFile::from_test_wasms(
        random_network_seed(),
        vec![TestWasm::MigrateNew],
        properties.try_into().unwrap(),
    )
    .await;

    // Choose to close the chain for the first version of the app
    let _: ActionHash = conductor
        .call(
            &alice_cell.zome(TestWasm::MigrateInitial),
            "close_chain_for_new",
            new_dna.dna_hash().clone(),
        )
        .await;

    // Install the new version of the app
    let app_new = conductor
        .setup_app_for_agent("app_2", alice.clone(), &[new_dna.clone()])
        .await
        .unwrap();

    let alice_cells = app_new.into_cells();
    let alice_cell = alice_cells.first().unwrap();

    // Create some data in the new version of the app
    let _: ActionHash = conductor
        .call(&alice_cell.zome(TestWasm::MigrateNew), "create", ())
        .await;

    // Now try to get all the data from the new version of the app, which is supposed to include the data from the first version
    let results: Vec<MyType> = conductor
        .call(
            &alice_cell.zome(TestWasm::MigrateNew),
            "get_all_my_types",
            (),
        )
        .await;

    assert_eq!(2, results.len());

    assert_eq!(results[0].value, "test");
    assert_eq!(results[0].amount, 0);

    assert_eq!(results[1].value, "test new");
    assert_eq!(results[1].amount, 4);
}
