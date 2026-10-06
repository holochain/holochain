use crate::conductor::api::error::ConductorApiError;
use crate::sweettest::{SweetConductor, SweetDnaFile};
use holo_hash::AgentPubKey;
use holochain_zome_types::prelude::*;
use matches::assert_matches;
use std::collections::HashSet;

/// The unrevoked direct signal grants committed on `cell_id`.
async fn direct_signal_grants(conductor: &SweetConductor, cell_id: &CellId) -> Vec<CapGrantInfo> {
    let cell_set: HashSet<CellId> = [cell_id.clone()].into_iter().collect();
    conductor
        .raw_handle()
        .capability_grant_info(&cell_set, false)
        .await
        .unwrap()
        .0
        .into_iter()
        .flat_map(|(_, grants)| grants)
        .filter(|info| info.cap_grant.capability == Capability::DirectSignal)
        .collect()
}

/// A DNA with no zomes has no coordinator to commit a grant from, so the conductor is the only
/// way for such a DNA to get one.
#[tokio::test(flavor = "multi_thread")]
async fn grant_direct_signal_capability_commits_a_direct_signal_grant() {
    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();

    let action_hash = conductor
        .raw_handle()
        .grant_direct_signal_capability(
            cell_id.clone(),
            "direct-signal".into(),
            GrantConstraint::Unrestricted,
        )
        .await
        .expect("granting a direct signal capability must succeed");

    let grants = direct_signal_grants(&conductor, &cell_id).await;
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].action_hash, action_hash);
    assert_eq!(grants[0].cap_grant.tag, "direct-signal");
}

#[tokio::test(flavor = "multi_thread")]
async fn grant_direct_signal_capability_for_app_rejects_a_cell_of_another_app() {
    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let app_1 = conductor.setup_app("app-1", [&dna]).await.unwrap();
    let app_2 = conductor.setup_app("app-2", [&dna]).await.unwrap();
    let cell_1 = app_1.cells()[0].cell_id().clone();
    let cell_2 = app_2.cells()[0].cell_id().clone();

    conductor
        .raw_handle()
        .grant_direct_signal_capability_for_app(
            &"app-1".to_string(),
            cell_1.clone(),
            "direct-signal".into(),
            GrantConstraint::Unrestricted,
        )
        .await
        .expect("an app must be able to grant for its own cell");
    assert_eq!(direct_signal_grants(&conductor, &cell_1).await.len(), 1);

    let err = conductor
        .raw_handle()
        .grant_direct_signal_capability_for_app(
            &"app-1".to_string(),
            cell_2.clone(),
            "direct-signal".into(),
            GrantConstraint::Unrestricted,
        )
        .await
        .expect_err("an app must not be able to grant for another app's cell");
    assert_matches!(
        err,
        ConductorApiError::Other(ref e) if e.to_string().contains("Cell not found in app")
    );
    assert!(
        direct_signal_grants(&conductor, &cell_2).await.is_empty(),
        "a rejected grant must not be committed"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn grant_direct_signal_capability_rejects_an_unknown_cell() {
    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let unknown_cell = CellId::new(
        app.cells()[0].dna_hash().clone(),
        AgentPubKey::from_raw_36(vec![9; 36]),
    );

    conductor
        .raw_handle()
        .grant_direct_signal_capability(
            unknown_cell,
            "direct-signal".into(),
            GrantConstraint::Unrestricted,
        )
        .await
        .expect_err("a grant for a cell that is not running must be rejected");
}

#[tokio::test(flavor = "multi_thread")]
async fn grant_direct_signal_capability_for_app_rejects_an_unknown_app() {
    let mut conductor = SweetConductor::standard().await;
    let dna = SweetDnaFile::unique_empty().await;
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();

    conductor
        .raw_handle()
        .grant_direct_signal_capability_for_app(
            &"no-such-app".to_string(),
            cell_id.clone(),
            "direct-signal".into(),
            GrantConstraint::Unrestricted,
        )
        .await
        .expect_err("a grant for an app that is not installed must be rejected");
    assert!(
        direct_signal_grants(&conductor, &cell_id).await.is_empty(),
        "a rejected grant must not be committed"
    );
}
