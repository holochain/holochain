use crate::conductor::CellError;
use hdk::prelude::AgentPubKeyFixturator;
use holo_hash::{ActionHash, AgentPubKey};
use holochain_state::prelude::SourceChainError;
use holochain_wasm_test_utils::TestWasm;
use matches::assert_matches;

use crate::core::workflow::WorkflowError;
use crate::{
    conductor::api::error::ConductorApiError,
    sweettest::{SweetConductor, SweetConductorConfig, SweetDnaFile},
};

#[tokio::test(flavor = "multi_thread")]
async fn is_same_agent_without_dpki() {
    let mut conductor =
        SweetConductor::from_config(SweetConductorConfig::standard().no_dpki()).await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentKeyLineage])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let agent_key = app.agent().clone();
    // Test wasm with a function to create an entry that contains two agent keys. The agent keys are
    // checked for `is_same_agent` during validation.
    // Without DPKI installed, the keys are compared for equality.
    let zome = app.cells()[0].zome(TestWasm::AgentKeyLineage.coordinator_zome_name());

    // Creating an entry with identical agent keys should succeed.
    let response: Result<ActionHash, _> = conductor
        .call_fallible(
            &zome,
            "create_entry_if_keys_of_same_lineage",
            (agent_key.clone(), agent_key.clone()),
        )
        .await;
    assert!(response.is_ok());

    // Creating an entry with two non-existing agent keys should succeed too. As there is no DPKI
    // to check for lineage, it just checks if the keys are identical.
    let fake_agent_key = ::fixt::fixt!(AgentPubKey);
    let response: Result<ActionHash, _> = conductor
        .call_fallible(
            &zome,
            "create_entry_if_keys_of_same_lineage",
            (fake_agent_key.clone(), fake_agent_key.clone()),
        )
        .await;
    assert!(response.is_ok());

    // Creating an entry with two different agent keys should fail.
    let response: Result<ActionHash, _> = conductor
        .call_fallible(
            &zome,
            "create_entry_if_keys_of_same_lineage",
            (agent_key.clone(), ::fixt::fixt!(AgentPubKey)),
        )
        .await;
    if let Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) = response {
        assert_matches!(
            *workflow_error,
            WorkflowError::SourceChainError(SourceChainError::InvalidCommit(e)) if e.contains("agent key is not of same lineage")
        );
    } else {
        panic!("expected workflow error");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn is_same_agent() {
    let mut conductor = SweetConductor::isolated_singleton().await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentKeyLineage])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let agent_key = app.agent().clone();
    // Test wasm with a function to create an entry that contains two agent keys. The agent keys are
    // checked for `is_same_agent` during validation.
    // Two keys of the same lineage will let validation pass.
    // Two keys that are not of the same lineage lets validation fail.
    let zome = app.cells()[0].zome(TestWasm::AgentKeyLineage.coordinator_zome_name());

    // Creating an entry with the two identical keys should succeed.
    let response: Result<ActionHash, _> = conductor
        .call_fallible(
            &zome,
            "create_entry_if_keys_of_same_lineage",
            (agent_key.clone(), agent_key.clone()),
        )
        .await;
    assert!(response.is_ok());

    // Creating an entry with the valid agent key and a fake agent key should fail, because the
    // fake key is not of the agent's key lineage.
    let response: Result<ActionHash, _> = conductor
        .call_fallible(
            &zome,
            "create_entry_if_keys_of_same_lineage",
            (agent_key.clone(), ::fixt::fixt!(AgentPubKey)),
        )
        .await;
    if let Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) = response {
        assert_matches!(*workflow_error, WorkflowError::SourceChainError(_));
    } else {
        panic!("expected workflow error");
    }

    // Creating an entry with a fake agent key twice should fail, because the
    // fake key is not registered in DPKI.
    let fake_agent_key = ::fixt::fixt!(AgentPubKey);
    let response: Result<ActionHash, _> = conductor
        .call_fallible(
            &zome,
            "create_entry_if_keys_of_same_lineage",
            (fake_agent_key.clone(), fake_agent_key.clone()),
        )
        .await;
    if let Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) = response {
        assert_matches!(*workflow_error, WorkflowError::SourceChainError(_));
    } else {
        panic!("expected workflow error");
    }

    // TODO: When adding a function to update an agent key to DPKI service, append to this test
    // a key update and make sure `create_entry_if_keys_of_same_lineage` succeeds for new agent key.
}

#[tokio::test(flavor = "multi_thread")]
async fn get_agent_key_lineage_during_init_without_dpki() {
    let mut conductor =
        SweetConductor::from_config(SweetConductorConfig::standard().no_dpki()).await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentKeyLineage])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let zome = app.cells()[0].zome(TestWasm::AgentKeyLineage.coordinator_zome_name());

    // Call a no op function that will only trigger init. Init gets key lineage and returns `Pass`
    // if successful and otherwise returns an error.
    let _: () = conductor.call(&zome, "no_op_init", ()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn get_agent_key_lineage_during_init() {
    let mut conductor = SweetConductor::from_config(SweetConductorConfig::standard()).await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentKeyLineage])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let zome = app.cells()[0].zome(TestWasm::AgentKeyLineage.coordinator_zome_name());

    // TODO: Update key first before calling init to make sure that get lineage call used DPKI and
    // returns two keys.

    // Call a no op function that will only trigger init. Init gets key lineage and returns `Pass`
    // if successful and otherwise returns an error.
    let _: () = conductor.call(&zome, "no_op_init", ()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn get_agent_key_lineage_without_dpki() {
    let mut conductor =
        SweetConductor::from_config(SweetConductorConfig::standard().no_dpki()).await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentKeyLineage])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let agent_key = app.agent().clone();
    let zome = app.cells()[0].zome(TestWasm::AgentKeyLineage.coordinator_zome_name());

    // Without DPKI, the lineage should just be the one agent key.
    let response: Vec<AgentPubKey> = conductor
        .call(&zome, "get_lineage_of_agent_keys", agent_key.clone())
        .await;
    assert_eq!(response, vec![agent_key.clone()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_agent_key_lineage() {
    let mut conductor = SweetConductor::from_config(SweetConductorConfig::standard()).await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentKeyLineage])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let agent_key = app.agent().clone();
    let zome = app.cells()[0].zome(TestWasm::AgentKeyLineage.coordinator_zome_name());

    // The lineage should just be the one agent key.
    let response: Vec<AgentPubKey> = conductor
        .call(&zome, "get_lineage_of_agent_keys", agent_key.clone())
        .await;
    assert_eq!(response, vec![agent_key.clone()]);

    // TODO: Update key and call get lineage of keys again.
}
