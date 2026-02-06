use super::validation_deps::SysValDeps;
use super::validation_deps::ValidationDependencies;
use crate::core::workflow::sys_validation_workflow::types::Outcome;
use crate::core::workflow::sys_validation_workflow::validate_op;
use crate::core::workflow::WorkflowResult;
use crate::prelude::*;
use ::fixt::prelude::*;
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holochain_cascade::CascadeSource;

/// Test that a valid ChainFork warrant is accepted when both actions:
/// - Have the same author
/// - Have the same prev_action (proving the fork)
/// - Have valid signatures
#[tokio::test(flavor = "multi_thread")]
async fn validate_chain_fork_warrant_accepted() {
    holochain_trace::test_run();

    let mut test_case = ChainForkWarrantTestCase::new().await;

    // Create the fork scenario: two actions with the same prev_action
    let (action1, action2, _prev_action_hash) = test_case.create_forking_actions().await;

    // Create a valid ChainFork warrant
    let warrant_op = test_case
        .create_chain_fork_warrant(&action1, &action2)
        .await;

    // Insert dependencies so validation can find the actions
    test_case.insert_action_dependency(&action1);
    test_case.insert_action_dependency(&action2);

    let outcome = test_case.validate_warrant(warrant_op).await.unwrap();

    assert!(
        matches!(outcome, Outcome::Accepted),
        "Expected Accepted but actual outcome was {outcome:?}"
    );
}

/// Test that a ChainFork warrant is rejected when chain_author doesn't match action1's author
#[tokio::test(flavor = "multi_thread")]
async fn validate_chain_fork_warrant_rejected_chain_author_mismatch() {
    holochain_trace::test_run();

    let mut test_case = ChainForkWarrantTestCase::new().await;

    // Create the fork scenario
    let (action1, action2, _) = test_case.create_forking_actions().await;

    // Create a warrant with a mismatched chain_author
    let wrong_author = test_case.keystore.new_sign_keypair_random().await.unwrap();
    let warrant_op = test_case
        .create_chain_fork_warrant_with_chain_author(&action1, &action2, wrong_author)
        .await;

    // Insert dependencies
    test_case.insert_action_dependency(&action1);
    test_case.insert_action_dependency(&action2);

    let outcome = test_case.validate_warrant(warrant_op).await.unwrap();

    match outcome {
        Outcome::Rejected(reason) => {
            assert!(
                reason.contains("chain author mismatch"),
                "Expected 'chain author mismatch' in rejection reason, got: {reason}"
            );
        }
        _ => panic!("Expected Rejected outcome but got: {outcome:?}"),
    }
}

/// Test that a ChainFork warrant is rejected when the two actions have different authors
#[tokio::test(flavor = "multi_thread")]
async fn validate_chain_fork_warrant_rejected_action_authors_differ() {
    holochain_trace::test_run();

    let mut test_case = ChainForkWarrantTestCase::new().await;

    // Create actions with different authors
    let (action1, action2) = test_case.create_actions_with_different_authors().await;

    // Create a warrant (using action1's author as chain_author)
    let warrant_op = test_case
        .create_chain_fork_warrant(&action1, &action2)
        .await;

    // Insert dependencies
    test_case.insert_action_dependency(&action1);
    test_case.insert_action_dependency(&action2);

    let outcome = test_case.validate_warrant(warrant_op).await.unwrap();

    match outcome {
        Outcome::Rejected(reason) => {
            assert!(
                reason.contains("action pair author mismatch"),
                "Expected 'action pair author mismatch' in rejection reason, got: {reason}"
            );
        }
        _ => panic!("Expected Rejected outcome but got: {outcome:?}"),
    }
}

/// Test that a ChainFork warrant is rejected when prev_action differs (not a real fork)
#[tokio::test(flavor = "multi_thread")]
async fn validate_chain_fork_warrant_rejected_prev_actions_differ() {
    holochain_trace::test_run();

    let mut test_case = ChainForkWarrantTestCase::new().await;

    // Create actions with different prev_actions (not a fork)
    let (action1, action2) = test_case.create_non_forking_actions().await;

    // Create a warrant
    let warrant_op = test_case
        .create_chain_fork_warrant(&action1, &action2)
        .await;

    // Insert dependencies
    test_case.insert_action_dependency(&action1);
    test_case.insert_action_dependency(&action2);

    let outcome = test_case.validate_warrant(warrant_op).await.unwrap();

    match outcome {
        Outcome::Rejected(reason) => {
            assert!(
                reason.contains("action pair seq mismatch"),
                "Expected 'action pair seq mismatch' in rejection reason, got: {reason}"
            );
        }
        _ => panic!("Expected Rejected outcome but got: {outcome:?}"),
    }
}

// Note: Testing bad action signatures within a ChainFork warrant is not covered here because:
// 1. Warrants with bad warrant-level signatures are caught by `verify_warrant_signature` before
//    they reach `validate_warrant_op` in the sys validation workflow
// 2. Bad action signatures within the warrant would hit the unreachable code path for
//    `CounterfeitAction` since counterfeit ops are expected to be filtered before sys validation
// The signature verification for actions in warrants is still exercised by the valid warrant test.

/// Test that a ChainFork warrant is rejected when the action pair has different authors.
///
/// Note: `detect_fork` now filters by author in its SQL query, so cross-author collisions
/// will not produce warrants in practice. This test verifies defense-in-depth: even if a
/// cross-author warrant were somehow constructed, the warrant validation correctly rejects it.
#[tokio::test(flavor = "multi_thread")]
async fn validate_chain_fork_warrant_rejected_cross_author_collision() {
    holochain_trace::test_run();

    let mut test_case = ChainForkWarrantTestCase::new().await;

    // Create the cross-author collision scenario:
    // Two actions from different authors pointing to the same prev_hash.
    let (action_agent_a, action_agent_b, _shared_prev_hash) =
        test_case.create_cross_author_collision().await;

    // Create a warrant as if detect_fork found action_agent_b when checking action_agent_a.
    // The warrant's chain_author is set to agent A (the author of the incoming action),
    // but the action pair contains actions from both agents.
    let warrant_op = test_case
        .create_chain_fork_warrant(&action_agent_a, &action_agent_b)
        .await;

    // Insert dependencies so validation can find both actions
    test_case.insert_action_dependency(&action_agent_a);
    test_case.insert_action_dependency(&action_agent_b);

    let outcome = test_case.validate_warrant(warrant_op).await.unwrap();

    // The warrant must be rejected because the two actions have different authors
    assert!(
        matches!(&outcome, Outcome::Rejected(reason) if reason.contains("action pair author mismatch")),
        "Expected Rejected with 'action pair author mismatch' but got: {outcome:?}"
    );
}

/// Test that a ChainFork warrant returns MissingDhtDep when forked actions are not available
#[tokio::test(flavor = "multi_thread")]
async fn validate_chain_fork_warrant_missing_dependency() {
    holochain_trace::test_run();

    let test_case = ChainForkWarrantTestCase::new().await;

    // Create the fork scenario
    let (action1, action2, _) = test_case.create_forking_actions().await;

    // Create a valid warrant but DON'T insert the action dependencies
    let warrant_op = test_case
        .create_chain_fork_warrant(&action1, &action2)
        .await;

    let outcome = test_case.validate_warrant(warrant_op).await.unwrap();

    assert!(
        matches!(outcome, Outcome::MissingDhtDep),
        "Expected MissingDhtDep but actual outcome was {outcome:?}"
    );
}

/// Test helper for ChainFork warrant validation tests
struct ChainForkWarrantTestCase {
    keystore: holochain_keystore::MetaLairClient,
    validation_dependencies: SysValDeps,
    chain_author: AgentPubKey,
    warrant_author: AgentPubKey,
    dna_def: DnaDef,
}

impl ChainForkWarrantTestCase {
    async fn new() -> Self {
        let keystore = holochain_keystore::test_keystore();
        let chain_author = keystore.new_sign_keypair_random().await.unwrap();
        let warrant_author = keystore.new_sign_keypair_random().await.unwrap();
        let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);

        Self {
            keystore,
            validation_dependencies: SysValDeps::default(),
            chain_author,
            warrant_author,
            dna_def,
        }
    }

    /// Create two actions that fork (same prev_action, same author, different content)
    async fn create_forking_actions(&self) -> (SignedActionHashed, SignedActionHashed, ActionHash) {
        let prev_action_hash = fixt!(ActionHash);

        // First action
        let mut create1 = fixt!(Create);
        create1.author = self.chain_author.clone();
        create1.prev_action = prev_action_hash.clone();
        create1.action_seq = 5;
        create1.timestamp = Timestamp::now();
        create1.entry_type = EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Public,
        });
        create1.entry_hash = fixt!(EntryHash);
        let action1 = self.sign_action(Action::Create(create1)).await;

        // Second action (different entry hash makes it a different action)
        let mut create2 = fixt!(Create);
        create2.author = self.chain_author.clone();
        create2.prev_action = prev_action_hash.clone();
        create2.action_seq = 5;
        create2.timestamp = Timestamp::now();
        create2.entry_type = EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Public,
        });
        create2.entry_hash = fixt!(EntryHash); // Different entry hash
        let action2 = self.sign_action(Action::Create(create2)).await;

        (action1, action2, prev_action_hash)
    }

    /// Create two actions with different authors (invalid fork scenario)
    async fn create_actions_with_different_authors(
        &self,
    ) -> (SignedActionHashed, SignedActionHashed) {
        let prev_action_hash = fixt!(ActionHash);
        let other_author = self.keystore.new_sign_keypair_random().await.unwrap();

        // First action with chain_author
        let mut create1 = fixt!(Create);
        create1.author = self.chain_author.clone();
        create1.prev_action = prev_action_hash.clone();
        create1.action_seq = 5;
        create1.timestamp = Timestamp::now();
        create1.entry_type = EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Public,
        });
        let action1 = self.sign_action(Action::Create(create1)).await;

        // Second action with different author
        let mut create2 = fixt!(Create);
        create2.author = other_author;
        create2.prev_action = prev_action_hash.clone();
        create2.action_seq = 5;
        create2.timestamp = Timestamp::now();
        create2.entry_type = EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Public,
        });
        let action2 = self.sign_action(Action::Create(create2)).await;

        (action1, action2)
    }

    /// Create a cross-author collision: two actions from different agents with same prev_hash.
    ///
    /// This creates two actions from different agents with the same `prev_hash`.
    /// While `detect_fork` now filters by author (preventing this from producing warrants),
    /// this helper is still useful for testing warrant validation defense-in-depth.
    async fn create_cross_author_collision(
        &self,
    ) -> (SignedActionHashed, SignedActionHashed, ActionHash) {
        let shared_prev_hash = fixt!(ActionHash);
        let other_agent = self.keystore.new_sign_keypair_random().await.unwrap();

        // Action from the test case's chain_author (Agent A)
        let mut create_agent_a = fixt!(Create);
        create_agent_a.author = self.chain_author.clone();
        create_agent_a.prev_action = shared_prev_hash.clone();
        create_agent_a.action_seq = 5;
        create_agent_a.timestamp = Timestamp::now();
        create_agent_a.entry_type = EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Public,
        });
        create_agent_a.entry_hash = fixt!(EntryHash);
        let action_agent_a = self.sign_action(Action::Create(create_agent_a)).await;

        // Action from a different agent (Agent B) with the SAME prev_hash
        let mut create_agent_b = fixt!(Create);
        create_agent_b.author = other_agent;
        create_agent_b.prev_action = shared_prev_hash.clone();
        create_agent_b.action_seq = 5;
        create_agent_b.timestamp = Timestamp::now();
        create_agent_b.entry_type = EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Public,
        });
        create_agent_b.entry_hash = fixt!(EntryHash);
        let action_agent_b = self.sign_action(Action::Create(create_agent_b)).await;

        (action_agent_a, action_agent_b, shared_prev_hash)
    }

    /// Create two actions with different prev_actions (not a real fork)
    async fn create_non_forking_actions(&self) -> (SignedActionHashed, SignedActionHashed) {
        // First action
        let mut create1 = fixt!(Create);
        create1.author = self.chain_author.clone();
        create1.prev_action = fixt!(ActionHash);
        create1.action_seq = 5;
        create1.timestamp = Timestamp::now();
        create1.entry_type = EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Public,
        });
        let action1 = self.sign_action(Action::Create(create1)).await;

        // Second action with DIFFERENT prev_action
        let mut create2 = fixt!(Create);
        create2.author = self.chain_author.clone();
        create2.prev_action = fixt!(ActionHash); // Different prev_action
        create2.action_seq = 5;
        create2.timestamp = Timestamp::now();
        create2.entry_type = EntryType::App(AppEntryDef {
            entry_index: 0.into(),
            zome_index: 0.into(),
            visibility: EntryVisibility::Public,
        });
        let action2 = self.sign_action(Action::Create(create2)).await;

        (action1, action2)
    }

    async fn sign_action(&self, action: Action) -> SignedActionHashed {
        use holochain_zome_types::action::ActionHashed;
        let action_hashed = ActionHashed::from_content_sync(action);
        SignedActionHashed::sign(&self.keystore, action_hashed)
            .await
            .unwrap()
    }

    /// Create a valid ChainFork warrant
    async fn create_chain_fork_warrant(
        &self,
        action1: &SignedActionHashed,
        action2: &SignedActionHashed,
    ) -> holochain_types::warrant::WarrantOp {
        let chain_author = action1.action().author().clone();
        self.create_chain_fork_warrant_with_chain_author(action1, action2, chain_author)
            .await
    }

    /// Create a ChainFork warrant with a specified chain_author
    async fn create_chain_fork_warrant_with_chain_author(
        &self,
        action1: &SignedActionHashed,
        action2: &SignedActionHashed,
        chain_author: AgentPubKey,
    ) -> holochain_types::warrant::WarrantOp {
        use holochain_zome_types::warrant::{ChainIntegrityWarrant, Warrant, WarrantProof};

        let warrant = Warrant::new(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
                chain_author: chain_author.clone(),
                action_pair: (
                    (action1.as_hash().clone(), action1.signature.clone()),
                    (action2.as_hash().clone(), action2.signature.clone()),
                ),
            }),
            self.warrant_author.clone(),
            Timestamp::now(),
            chain_author,
        );

        holochain_types::warrant::WarrantOp::sign(&self.keystore, warrant)
            .await
            .unwrap()
    }

    /// Insert an action into the validation dependencies
    fn insert_action_dependency(&mut self, action: &SignedActionHashed) {
        use super::validation_deps::{ValidationDependencyState, ValidationDependencyValue};

        let mut deps = self.validation_dependencies.lock().expect("poisoned");
        let state = ValidationDependencyState::new_present(
            ValidationDependencyValue::Action(action.clone()),
            CascadeSource::Local,
        );
        let new_deps =
            ValidationDependencies::new_from_iter(vec![(action.as_hash().clone(), state)]);
        deps.merge(new_deps);
    }

    async fn validate_warrant(
        &self,
        warrant_op: holochain_types::warrant::WarrantOp,
    ) -> WorkflowResult<Outcome> {
        let dna_hash = DnaDefHashed::from_content_sync(self.dna_def.clone()).hash;
        let op = DhtOp::WarrantOp(Box::new(warrant_op));

        validate_op(&op, &dna_hash, self.validation_dependencies.clone()).await
    }
}
