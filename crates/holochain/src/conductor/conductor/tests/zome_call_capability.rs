use crate::conductor::api::error::ConductorApiError;
use crate::core::workflow::WorkflowError;
use crate::sweettest::{SweetConductor, SweetDnaFile, SweetInlineZomes};
use ::fixt::fixt;
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain_state::source_chain::SourceChainError;
use holochain_zome_types::prelude::*;
use matches::assert_matches;
use std::collections::{BTreeSet, HashSet};

/// Build the cap grant used by the closed-chain tests.
fn test_cap_grant() -> ZomeCallCapGrant {
    let mut functions = HashSet::new();
    let granted_function: GrantedFunction = ("create_entry".into(), "get_entry".into());
    functions.insert(granted_function);
    let mut assignees = BTreeSet::new();
    assignees.insert(fixt!(AgentPubKey, ::fixt::Predictable, 1));

    ZomeCallCapGrant {
        tag: "signing_key".into(),
        access: CapAccess::Assigned {
            secret: [0; 64].into(),
            assignees,
        },
        functions: GrantedFunctions::Listed(functions),
    }
}

/// Set up a conductor with a cell whose coordinator zome can close its own chain.
async fn conductor_with_closable_chain() -> (SweetConductor, CellId) {
    let zomes = SweetInlineZomes::new(vec![], 0).function("close_chain", |api, ()| {
        Ok(api.close_chain(CloseChainInput { new_target: None })?)
    });
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();
    (conductor, cell_id)
}

async fn close_chain(conductor: &SweetConductor, cell_id: &CellId) {
    let cell = conductor
        .get_sweet_cell(cell_id.clone())
        .expect("cell must exist");
    let _: ActionHash = conductor
        .call(&cell.zome(SweetInlineZomes::COORDINATOR), "close_chain", ())
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn revoke_zome_call_capability_fails_on_closed_chain() {
    let (conductor, cell_id) = conductor_with_closable_chain().await;

    // Granting a capability on the open chain succeeds.
    let grant_action_hash = conductor
        .raw_handle()
        .grant_zome_call_capability(GrantZomeCallCapabilityPayload {
            cell_id: cell_id.clone(),
            cap_grant: test_cap_grant(),
        })
        .await
        .expect("granting a capability on an open chain must succeed");

    // Close the chain, so that no more actions may be committed to it.
    close_chain(&conductor, &cell_id).await;

    // Revoking the capability must fail self-validation because the chain is closed.
    let err = conductor
        .raw_handle()
        .revoke_zome_call_capability(cell_id, grant_action_hash)
        .await
        .expect_err("revoking a capability on a closed chain must fail");

    assert_matches!(
        err,
        ConductorApiError::WorkflowError(WorkflowError::SourceChainError(
            SourceChainError::InvalidCommit(ref reason)
        )) if reason.contains("No more actions are allowed after a chain close")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn grant_zome_call_capability_fails_on_closed_chain() {
    let (conductor, cell_id) = conductor_with_closable_chain().await;

    // Close the chain, so that no more actions may be committed to it.
    close_chain(&conductor, &cell_id).await;

    // Granting a capability must fail self-validation because the chain is closed.
    let err = conductor
        .raw_handle()
        .grant_zome_call_capability(GrantZomeCallCapabilityPayload {
            cell_id,
            cap_grant: test_cap_grant(),
        })
        .await
        .expect_err("granting a capability on a closed chain must fail");

    assert_matches!(
        err,
        ConductorApiError::WorkflowError(WorkflowError::SourceChainError(
            SourceChainError::InvalidCommit(ref reason)
        )) if reason.contains("No more actions are allowed after a chain close")
    );
}
