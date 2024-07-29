use crate::conductor::CellError;
use hdk::prelude::AgentPubKeyFixturator;
use holo_hash::ActionHash;
use holochain_state::prelude::SourceChainError;
use holochain_wasm_test_utils::TestWasm;
use matches::assert_matches;

use crate::core::workflow::WorkflowError;
use crate::{
    conductor::api::error::ConductorApiError,
    sweettest::{SweetConductor, SweetConductorConfig, SweetDnaFile},
};

#[tokio::test(flavor = "multi_thread")]
async fn is_same_agent_without_dpki_installation() {
    let mut conductor =
        SweetConductor::from_config(SweetConductorConfig::standard().no_dpki()).await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentLineage])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let agent_key = app.agent().clone();
    // Test wasm with a function to create an entry that contains an agent key. That agent key is
    // checked if `is_same_agent` during validation.
    // Without DPKI installed, the keys are compared for equality.
    let zome = app.cells()[0].zome(TestWasm::AgentLineage.coordinator_zome_name());

    // Creating an entry with the author's agent key should succeed.
    let response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "create_entry_if_same_agent", agent_key.clone())
        .await;
    assert!(response.is_ok());

    // Creating an entry with a fake agent key should fail, because the key is not of the
    // agent's key lineage.
    let response: Result<ActionHash, _> = conductor
        .call_fallible(
            &zome,
            "create_entry_if_same_agent",
            ::fixt::fixt!(AgentPubKey),
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
    let mut conductor = SweetConductor::from_standard_config().await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentLineage])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let agent_key = app.agent().clone();
    // Test wasm with a function to create an entry that contains an agent key. That agent key is
    // checked if `is_same_agent` during validation.
    // A key of the same lineage as the entry author lets validation pass.
    // A key that is not of the same lineage as the entry author lets validation fail.
    let zome = app.cells()[0].zome(TestWasm::AgentLineage.coordinator_zome_name());

    // Creating an entry with the author's agent key should succeed.
    let response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "create_entry_if_same_agent", agent_key.clone())
        .await;
    assert!(response.is_ok());

    // Creating an entry with a fake agent key should fail, because the key is not of the
    // agent's key lineage.
    let response: Result<ActionHash, _> = conductor
        .call_fallible(
            &zome,
            "create_entry_if_same_agent",
            ::fixt::fixt!(AgentPubKey),
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

    // TODO: When adding function to update an agent key to DPKI service, append to this test
    // a key update and make sure `create_entry_if_same_agent` succeeds for new agent key.
}
