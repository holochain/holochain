use crate::conductor::CellError;
use hdk::prelude::AgentPubKeyFixturator;
use holo_hash::ActionHash;
use holochain_state::prelude::SourceChainError;
use holochain_wasm_test_utils::TestWasm;
use matches::assert_matches;

use crate::core::workflow::WorkflowError;
use crate::{
    conductor::api::error::ConductorApiError,
    sweettest::{SweetConductor, SweetDnaFile},
};

#[tokio::test(flavor = "multi_thread")]
async fn is_same_agent() {
    println!("hello");
    let mut conductor = SweetConductor::from_standard_config().await;
    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentLineage])
        .await
        .0;
    let app = conductor.setup_app("", &[dna_file]).await.unwrap();
    let agent = app.agent().clone();
    // println!("app installed {app:?}");
    let zome = app.cells()[0].zome(TestWasm::AgentLineage.coordinator_zome_name());
    let response: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "create_entry_if_same_agent", agent.clone())
        .await;
    println!("response {response:?}");
    assert!(response.is_ok());

    // let dpki = conductor.running_services().dpki.unwrap();
    // dpki.state().await.

    let response: Result<ActionHash, _> = conductor
        .call_fallible(
            &zome,
            "create_entry_if_same_agent",
            ::fixt::fixt!(AgentPubKey),
        )
        .await;
    println!("response {response:?}");
    if let Err(ConductorApiError::CellError(CellError::WorkflowError(workflow_error))) = response {
        assert_matches!(
            *workflow_error,
            WorkflowError::SourceChainError(SourceChainError::InvalidCommit(e)) if e == "agent key is not of same lineage"
        );
    } else {
        panic!("expected workflow error");
    }
}
