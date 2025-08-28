use hdk::prelude::{Op, ValidateCallbackResult};
use holochain::sweettest::{
    await_consistency, SweetConductorBatch, SweetConductorConfig, SweetDnaFile, SweetInlineZomes,
};
use holochain_state::query::{CascadeTxnWrapper, Store};

// Test that warrants issuance can be disabled. This is useful for testing that
// warrants are propagated through publish and gossip.
//
// Alice creates invalid ops, Bob receives them but should not issue any warrant.
#[tokio::test(flavor = "multi_thread")]
async fn invalid_op_warrant_issuance_can_be_disabled() {
    holochain_trace::test_run();

    let zome_common = SweetInlineZomes::new(vec![], 0);
    let zome_without_validation = zome_common
        .clone()
        .integrity_function("validate", move |_api, _: Op| {
            Ok(ValidateCallbackResult::Valid)
        });
    // Any action is invalid, including genesis actions.
    let zome_with_validation = zome_common
        .clone()
        .integrity_function("validate", move |_api, _: Op| {
            Ok(ValidateCallbackResult::Invalid("nope".to_string()))
        });

    let network_seed = "seed".to_string();
    let (dna_without_validation, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_without_validation).await;
    let (dna_with_validation, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_with_validation).await;
    assert_eq!(
        dna_without_validation.dna_hash(),
        dna_with_validation.dna_hash()
    );
    let dna_hash = dna_without_validation.dna_hash();

    let config = SweetConductorConfig::rendezvous(true);
    // Disable warrants on Bob's conductor.
    let config_no_warranting = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|tc| tc.disable_warrant_issuance = true);
    let mut conductors =
        SweetConductorBatch::from_configs_rendezvous([config, config_no_warranting]).await;
    let (alice,) = conductors[0]
        .setup_app("test_app", [&dna_without_validation])
        .await
        .unwrap()
        .into_tuple();
    let (bob,) = conductors[1]
        .setup_app("test_app", [&dna_with_validation])
        .await
        .unwrap()
        .into_tuple();

    await_consistency(10, [&alice, &bob]).await.unwrap();

    // Bob must not have issued a warrant against Alice.
    // A warrant would have been created as part of app validating all of Alice's
    // ops, so once consistency is reached, the authored DB can be checked
    // for warrants.
    let alice_pubkey = alice.agent_pubkey().clone();
    conductors[1]
        .get_spaces()
        .get_all_authored_dbs(dna_hash)
        .unwrap()[0]
        .test_read(move |txn| {
            let store = CascadeTxnWrapper::from(txn);
            // TODO: check_valid here should be removed once warrants are validated.
            let warrants = store.get_warrants_for_agent(&alice_pubkey, false).unwrap();
            assert!(warrants.is_empty());
        });
}
