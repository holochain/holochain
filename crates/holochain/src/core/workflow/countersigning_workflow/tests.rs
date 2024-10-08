use crate::conductor::space::TestSpace;
use crate::core::queue_consumer::{TriggerReceiver, TriggerSender};
use crate::core::ribosome::weigh_placeholder;
use crate::core::workflow::countersigning_workflow::{
    accept_countersigning_request, countersigning_workflow, CountersigningSessionState,
    CountersigningWorkspace, SessionCompletionDecision, SessionResolutionSummary,
};
use crate::core::workflow::countersigning_workflow::{countersigning_success, WorkComplete};
use crate::core::workflow::WorkflowResult;
use crate::prelude::CreateFixturator;
use crate::prelude::EntryFixturator;
use crate::prelude::SignatureFixturator;
use crate::prelude::SignedAction;
use crate::prelude::{ActionBase, PreflightBytes, PreflightRequest, PreflightRequestAcceptance};
use crate::prelude::{ActionHashed, CounterSigningAgentState, DhtDbQueryCache, SignedActionHashed};
use fixt::prelude::*;
use hdk::prelude::{Action, Entry, EntryTypeFixturator, Record};
use hdk::prelude::{CounterSigningSessionTimes, Timestamp};
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holo_hash::ActionHash;
use holo_hash::{AgentPubKey, DnaHash, EntryHash};
use holochain_keystore::MetaLairClient;
use holochain_p2p::{HolochainP2pError, MockHolochainP2pDnaT};
use holochain_state::chain_lock::get_chain_lock;
use holochain_state::prelude::{
    chain_head_db, current_countersigning_session, remove_countersigning_session,
    set_withhold_publish, AppEntryBytesFixturator, HeadInfo,
};
use holochain_state::prelude::{
    insert_action, insert_entry, insert_op_dht, unlock_chain, CounterSigningSessionData,
};
use holochain_state::prelude::{StateMutationError, StateMutationResult};
use holochain_state::query::from_blob;
use holochain_state::source_chain;
use holochain_types::activity::AgentActivityResponse;
use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
use holochain_types::prelude::SystemSignal;
use holochain_types::prelude::{ChainItems, SignedActionHashedExt};
use holochain_types::signal::Signal;
use holochain_zome_types::cell::CellId;
use holochain_zome_types::countersigning::PreflightResponse;
use holochain_zome_types::prelude::CreateBase;
use holochain_zome_types::query::{ChainHead, ChainStatus};
use matches::assert_matches;
use std::ops::Add;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::{Receiver, Sender};

#[tokio::test(flavor = "multi_thread")]
async fn accept_countersigning_request_creates_state() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(60), &bob);
    test_harness
        .accept_countersigning_request(request)
        .await
        .unwrap();

    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn duplicate_accepts_do_not_overwrite_state() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request1 = test_preflight_request(&test_harness, Duration::from_secs(60), &bob);
    test_harness
        .accept_countersigning_request(request1.clone())
        .await
        .unwrap();

    let carol = test_harness.new_remote_agent().await;
    let request2 =
        test_preflight_request(&test_harness, std::time::Duration::from_secs(60), &carol);
    test_harness
        .accept_countersigning_request(request2)
        .await
        .unwrap_err();

    test_harness.expect_chain_locked().await;
    let accepted_session = test_harness.expect_session_accepted();
    assert_eq!(request1, accepted_session);
}

#[tokio::test(flavor = "multi_thread")]
async fn countersigning_session_expiry_from_accepted_with_no_commit() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    test_harness
        .accept_countersigning_request(request)
        .await
        .unwrap();

    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;

    // Accept should have triggered the workflow, respond to that run
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // State shouldn't change, just a callback registered to trigger the workflow on expiry
    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;
    test_harness.expect_no_pending_signals();

    // Wait for the workflow to run itself again on expiry
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;
    test_harness.expect_abandoned_signal().await;

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn chain_unlocked_outside_workflow() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    test_harness
        .accept_countersigning_request(request)
        .await
        .unwrap();

    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;

    // Simulate what would happen on a failed commit, the chain gets unlocked and the countersigning
    // workflow must be triggered
    test_harness.unlock_chain().await;
    test_harness.countersigning_tx.trigger(&"test");

    // The refresh mechanism should spot the missing chain lock
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // and terminate the session
    test_harness.expect_abandoned_signal().await;

    test_harness.expect_empty_workspace();
    test_harness.expect_no_pending_signals();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn chain_unlocked_outside_workflow_then_restart() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    test_harness
        .accept_countersigning_request(request)
        .await
        .unwrap();

    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;

    // Simulate what would happen on a failed commit, the chain gets unlocked and the countersigning
    // workflow must be triggered
    test_harness.unlock_chain().await;

    // Now simulate a restart, to check that Holochain will still recover even if it loses its state
    // at this point
    test_harness.clear_workspace_session();

    test_harness.countersigning_tx.trigger(&"test");
    // The refresh should have nothing to find because the lock is gone and nothing has been committed
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_empty_workspace();
    test_harness.expect_no_pending_signals();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn discard_session_with_lock_but_no_state() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    test_harness
        .accept_countersigning_request(request)
        .await
        .unwrap();

    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;

    // Simulate approximately what would happen on a restart. The session is lost from memory but
    // the chain is still locked.
    test_harness.clear_workspace_session();

    // Run the workflow on init
    test_harness.countersigning_tx.trigger(&"init");
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // The session state is lost, and we haven't published anything, so the session should be abandoned.
    // We don't get a signal in this case, so we just have to check that the chain gets unlocked.
    test_harness.expect_chain_unlocked().await;

    test_harness.expect_empty_workspace();
    test_harness.expect_no_pending_signals();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn receive_signatures_and_complete() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(60), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    let signatures = vec![
        bob.produce_signature(&session_data, &entry_hash, test_harness.keystore.clone())
            .await,
        test_harness
            .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
            .await,
    ];

    // Expect to receive a publish event.
    test_harness.reconfigure_network(|mut net| {
        net.expect_chc().return_once(|| None);
        net.expect_publish_countersign()
            .return_once(|_, _, _| Ok(()));
        net
    });

    // Receive the signatures as though they were coming in from a witness.
    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        signatures,
        test_harness.countersigning_tx.clone(),
    )
    .await;

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // One run should be enough when we got valid signatures and the session is now completed.
    test_harness.expect_success_signal().await;
    test_harness.expect_publish_and_integrate();

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn receive_valid_and_invalid_signatures_and_complete() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(60), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    let bob_invalid_sig = bob
        .produce_signature(&session_data, &entry_hash, test_harness.keystore.clone())
        .await;
    let bob_invalid_sig = SignedAction::new(bob_invalid_sig.into_data(), fixt!(Signature));
    let invalid_signatures = vec![
        bob_invalid_sig,
        test_harness
            .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
            .await,
    ];

    // Receive the invalid signatures to prove they are invalid.
    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        invalid_signatures.clone(),
        test_harness.countersigning_tx.clone(),
    )
    .await;

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_session_in_signatures_collected();

    let valid_signatures = vec![
        bob.produce_signature(&session_data, &entry_hash, test_harness.keystore.clone())
            .await,
        test_harness
            .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
            .await,
    ];

    // Expect to receive a publish event.
    test_harness.reconfigure_network(|mut net| {
        net.expect_chc().return_once(|| None);
        net.expect_publish_countersign()
            .return_once(|_, _, _| Ok(()));
        net
    });

    // Receive the signatures as though they were coming in from a witness.
    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        invalid_signatures,
        test_harness.countersigning_tx.clone(),
    )
    .await;
    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        valid_signatures,
        test_harness.countersigning_tx.clone(),
    )
    .await;

    // Should see both the invalid and the valid signatures in the same workflow run.
    // The invalid signature bundle should be ignored without causing an error and the second
    // bundle should be accepted.
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // One run should be enough when we got valid signatures and the session is now completed.
    test_harness.expect_success_signal().await;
    test_harness.expect_publish_and_integrate();

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

// Checks that when there are multiple authorities returning signature bundles and multiple sessions
// happening between a pair of agents, the extra signature bundles beyond the first one received
// are correctly handled. I.e. receiving further signature bundles after a new session has started
// will not impact the new session.
#[tokio::test(flavor = "multi_thread")]
async fn ignore_signature_bundles_from_previous_session() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    // Prepare network mock to expect two sessions to complete during this test
    test_harness.reconfigure_network({
        move |mut net| {
            net.expect_chc().times(2).returning(|| None);

            net.expect_publish_countersign()
                .times(2)
                .returning(|_, _, _| Ok(()));

            net
        }
    });

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(60), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    // Receive the signatures for this session.
    let signatures = vec![
        bob.produce_signature(&session_data, &entry_hash, test_harness.keystore.clone())
            .await,
        test_harness
            .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
            .await,
    ];
    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        signatures.clone(),
        test_harness.countersigning_tx.clone(),
    )
    .await;

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // The session gets completed by the first signature bundle we received
    test_harness.expect_success_signal().await;
    test_harness.expect_publish_and_integrate();
    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();

    // Now we can start a new session
    let new_request = test_preflight_request(&test_harness, Duration::from_secs(60), &bob);
    let new_my_acceptance = test_harness
        .accept_countersigning_request(new_request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let new_bob_acceptance = bob
        .accept_preflight_request(new_request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) = test_harness.build_session_data(
        new_request.clone(),
        vec![new_my_acceptance, new_bob_acceptance],
    );

    test_harness.expect_session_accepted();

    // Receive the previous signatures again, they should be ignored.
    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        signatures.clone(),
        test_harness.countersigning_tx.clone(),
    )
    .await;

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // Is this ideal? We're accepting the signatures
    test_harness.expect_session_in_signatures_collected();

    // Receive the right signatures for this session.
    let new_signatures = vec![
        bob.produce_signature(&session_data, &entry_hash, test_harness.keystore.clone())
            .await,
        test_harness
            .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
            .await,
    ];

    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        new_signatures.clone(),
        test_harness.countersigning_tx.clone(),
    )
    .await;

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // Now the session should complete with the new signatures
    test_harness.expect_success_signal().await;
    test_harness.expect_publish_and_integrate();
    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn attempts_resolution_if_only_invalid_signatures_received() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    let bob_invalid_sig = bob
        .produce_signature(&session_data, &entry_hash, test_harness.keystore.clone())
        .await;
    let bob_invalid_sig = SignedAction::new(bob_invalid_sig.into_data(), fixt!(Signature));
    let invalid_signatures = vec![
        bob_invalid_sig,
        test_harness
            .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
            .await,
    ];

    // Receive the invalid signature bundle.
    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        invalid_signatures.clone(),
        test_harness.countersigning_tx.clone(),
    )
    .await;

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // Saw the signatures but didn't accept them. We also haven't reached the end time yet so we
    // don't go straight to resolution
    test_harness.expect_session_in_signatures_collected();

    // Have Bob's authorities now show any activity yet.
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    // Should run again at timeout and attempt to resolve the session
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_abandoned_signal().await;
    test_harness.expect_session_removed(request).await;

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn recover_from_commit_when_other_agent_abandons() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Now, don't send signatures to our agent.

    // Provide no information about Bob's chain when resolution runs
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    // Run our workflow, which should trigger itself to spot the timed out session it will make a
    // single recovery attempt that will be indeterminate, and then abandon the session.
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_abandoned_signal().await;
    test_harness.expect_session_removed(request).await;

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn recover_after_restart_from_commit_when_other_agent_abandons() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Simulate a restart by wiping the workspace
    test_harness.clear_workspace_session();

    // Now for the sake of recovery, let's suppose that we can initially find no activity for Bob.
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    // Run our workflow, which should trigger itself to spot the timed out session and move it to
    // the unknown state for recovery.
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // This is where we'll stay unless Bob takes some action
    let resolution = test_harness.expect_session_in_unknown_state();
    assert_eq!(1, resolution.attempts);
    assert_eq!(1, resolution.outcomes.len());
    let bob_resolution = &resolution.outcomes[0];
    assert_eq!(bob.agent, bob_resolution.agent);
    assert_eq!(3, bob_resolution.decisions.len());
    assert!(bob_resolution
        .decisions
        .iter()
        .all(|d| *d == SessionCompletionDecision::Indeterminate));

    // Now let's help the recovery, Bob publishes some other activity
    let activity_response = bob.other_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    // Run the workflow again, this time we should spot the new activity and abandon the session
    test_harness.countersigning_tx.trigger(&"test");
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_abandoned_signal().await;
    test_harness.expect_session_removed(request).await;

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn recover_after_restart_from_commit_when_other_agent_completes() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Simulate a restart by wiping the workspace
    test_harness.clear_workspace_session();

    // Initially, find no data for Bob
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_session_in_unknown_state();

    test_harness.expect_session_in_unknown_state();

    // Now Bob's completed session shows up with an AAA
    let activity_response = bob
        .complete_session_activity_response(
            &session_data,
            entry.clone(),
            &entry_hash,
            test_harness.keystore.clone(),
            true,
        )
        .await;
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net.expect_chc().return_once(|| None);

            net.expect_publish_countersign()
                .return_once(|_, _, _| Ok(()));

            net
        }
    });

    test_harness.countersigning_tx.trigger(&"test");
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_success_signal().await;
    test_harness.expect_publish_and_integrate();

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn stay_in_unknown_state_when_activity_authorities_do_not_agree() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Simulate a restart to enter the unknown state on the next run.
    test_harness.clear_workspace_session();

    // Simulate mixed responses from AAAs. This is not really expected unless nodes are misbehaving
    // but if it does happen then we should stay in the unknown state.
    let assorted_responses = vec![
        bob.other_activity_response(),
        bob.complete_session_activity_response(
            &session_data,
            entry.clone(),
            &entry_hash,
            test_harness.keystore.clone(),
            true,
        )
        .await,
    ];
    test_harness.reconfigure_network({
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            let pick_response = Arc::new(AtomicUsize::new(0));
            net.expect_get_agent_activity().returning({
                let pick_response = pick_response.clone();
                let assorted_responses = assorted_responses.clone();
                move |_, _, _| {
                    let pick = pick_response.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        % assorted_responses.len();
                    Ok(vec![assorted_responses[pick].clone()])
                }
            });

            net
        }
    });

    for i in 1..5 {
        test_harness
            .respond_to_countersigning_workflow_signal()
            .await;
        test_harness.countersigning_tx.trigger(&"test");

        let resolution = test_harness.expect_session_in_unknown_state();

        assert_eq!(i, resolution.attempts);

        let some_complete = resolution.outcomes.iter().all(|o| {
            o.decisions
                .iter()
                .any(|d| matches!(d, SessionCompletionDecision::Complete(_)))
        });
        assert!(some_complete);
        let some_abandoned = resolution.outcomes.iter().all(|o| {
            o.decisions
                .iter()
                .any(|d| matches!(d, SessionCompletionDecision::Abandoned))
        });
        assert!(some_abandoned);
        let some_indeterminate = resolution.outcomes.iter().any(|o| {
            o.decisions
                .iter()
                .any(|d| matches!(d, SessionCompletionDecision::Indeterminate))
        });
        assert!(!some_indeterminate);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stay_in_unknown_state_when_activity_authorities_are_missing_data() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Simulate a restart to enter the unknown state on the next run.
    test_harness.clear_workspace_session();

    // Simulate mixed responses from AAAs. This is not really expected unless nodes are misbehaving
    // but if it does happen then we should stay in the unknown state.
    let assorted_responses = vec![
        bob.no_activity_response(),
        bob.complete_session_activity_response(
            &session_data,
            entry.clone(),
            &entry_hash,
            test_harness.keystore.clone(),
            true,
        )
        .await,
    ];
    test_harness.reconfigure_network({
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            let pick_response = Arc::new(AtomicUsize::new(0));
            net.expect_get_agent_activity().returning({
                let pick_response = pick_response.clone();
                let assorted_responses = assorted_responses.clone();
                move |_, _, _| {
                    let pick = pick_response.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        % assorted_responses.len();
                    Ok(vec![assorted_responses[pick].clone()])
                }
            });

            net
        }
    });

    for i in 1..5 {
        test_harness
            .respond_to_countersigning_workflow_signal()
            .await;
        test_harness.countersigning_tx.trigger(&"test");

        let resolution = test_harness.expect_session_in_unknown_state();

        assert_eq!(i, resolution.attempts);

        let some_complete = resolution.outcomes.iter().all(|o| {
            o.decisions
                .iter()
                .any(|d| matches!(d, SessionCompletionDecision::Complete(_)))
        });
        assert!(some_complete);
        let some_abandoned = resolution.outcomes.iter().any(|o| {
            o.decisions
                .iter()
                .any(|d| matches!(d, SessionCompletionDecision::Abandoned))
        });
        assert!(!some_abandoned);
        let some_indeterminate = resolution.outcomes.iter().all(|o| {
            o.decisions
                .iter()
                .any(|d| matches!(d, SessionCompletionDecision::Indeterminate))
        });
        assert!(some_indeterminate);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stay_in_unknown_state_when_bad_signatures_are_fetched() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Simulate a restart to enter the unknown state on the next run.
    test_harness.clear_workspace_session();

    // Simulate mixed responses from AAAs. This is not really expected unless nodes are misbehaving
    // but if it does happen then we should stay in the unknown state.
    let assorted_responses = vec![
        bob.complete_session_activity_response(
            &session_data,
            entry.clone(),
            &entry_hash,
            test_harness.keystore.clone(),
            false,
        )
        .await,
    ];
    test_harness.reconfigure_network({
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            // TODO only one available!
            let pick_response = Arc::new(AtomicUsize::new(0));
            net.expect_get_agent_activity().returning({
                let pick_response = pick_response.clone();
                let assorted_responses = assorted_responses.clone();
                move |_, _, _| {
                    let pick = pick_response.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        % assorted_responses.len();
                    Ok(vec![assorted_responses[pick].clone()])
                }
            });

            net
        }
    });

    for i in 1..5 {
        test_harness
            .respond_to_countersigning_workflow_signal()
            .await;
        test_harness.countersigning_tx.trigger(&"test");

        let resolution = test_harness.expect_session_in_unknown_state();
        assert_eq!(i, resolution.attempts);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn timeout_during_accept_does_not_interfere_with_previous_session() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(60), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    let signatures = vec![
        bob.produce_signature(&session_data, &entry_hash, test_harness.keystore.clone())
            .await,
        test_harness
            .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
            .await,
    ];

    // Expect to receive a publish event.
    test_harness.reconfigure_network(|mut net| {
        net.expect_chc().return_once(|| None);
        net.expect_publish_countersign()
            .return_once(|_, _, _| Ok(()));
        net
    });

    // Receive the signatures as though they were coming in from a witness.
    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        signatures,
        test_harness.countersigning_tx.clone(),
    )
    .await;

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // One run should be enough when we got valid signatures and the session is now completed.
    test_harness.expect_success_signal().await;
    test_harness.expect_publish_and_integrate();

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();

    let chain_head_after_first_session = test_harness.read_chain_head_hash().await;

    // Now, start a new session
    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    // Run once to schedule the timeout trigger
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // Now, simulate a timeout
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // The session should be abandoned
    test_harness.expect_abandoned_signal().await;

    let chain_head_after_second_session = test_harness.read_chain_head_hash().await;

    // The chain head should not have changed. This is exactly what we'd expect of course, but it's
    // important that the session abandon for the second session didn't mix up the countersigning
    // entry that's still at the chain head from the previous session.
    assert_eq!(
        chain_head_after_first_session,
        chain_head_after_second_session
    );

    // Try to force removing the chain head, just to prove that even if we had a bug in the check
    // that prevented the issue above, then we would reject the database mutation.
    let result = test_harness
        .try_remove_countersigning_entry(chain_head_after_first_session.action.clone())
        .await;
    assert_matches!(result, Err(StateMutationError::CannotRemoveFullyPublished));

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn respect_retry_limit_on_timeout_with_no_signatures_received() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, Some(3)).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Now we're in the accepted state and have committed but haven't received any signatures yet.
    test_harness.expect_session_accepted();

    // Have Bob's authorities now show any activity.
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    for _ in 0..2 {
        test_harness
            .respond_to_countersigning_workflow_signal()
            .await;

        test_harness.expect_session_in_unknown_state();
    }

    // Trying again should now abandon the session
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_abandoned_signal().await;
    test_harness.expect_session_removed(request).await;

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn respect_unlimited_retries_on_timeout_with_no_signatures_received() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, Some(0)).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Now we're in the accepted state and have committed but haven't received any signatures yet.
    test_harness.expect_session_accepted();

    // Have Bob's authorities now show any activity.
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    for _ in 0..10 {
        test_harness
            .respond_to_countersigning_workflow_signal()
            .await;

        test_harness.expect_session_in_unknown_state();
    }

    // And on and on and on...
    test_harness.expect_session_in_unknown_state();
}

#[tokio::test(flavor = "multi_thread")]
async fn retry_limit_does_not_apply_after_a_restart() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    // Configure 3 retries for sessions that time out
    let mut test_harness = TestHarness::new(dna_hash, Some(3)).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Simulate a restart
    test_harness.clear_workspace_session();

    // Have Bob's authorities now show any activity.
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    // Run the workflow on init
    test_harness.countersigning_tx.trigger(&"init");
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    for _ in 0..10 {
        test_harness
            .respond_to_countersigning_workflow_signal()
            .await;

        test_harness.expect_session_in_unknown_state();
    }

    // And on and on and on...
    test_harness.expect_session_in_unknown_state();
}

#[tokio::test(flavor = "multi_thread")]
async fn network_errors_do_not_count_towards_retry_limit() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, Some(3)).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Now we're in the accepted state and have committed but haven't received any signatures yet.
    test_harness.expect_session_accepted();

    // Have Bob's authorities return an error
    test_harness.reconfigure_network({
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity()
                .returning(move |_, _, _| Err(HolochainP2pError::Other("test".into())));

            net
        }
    });

    for _ in 0..10 {
        test_harness
            .respond_to_countersigning_workflow_signal()
            .await;

        let summary = test_harness.expect_session_in_unknown_state();
        assert_eq!(0, summary.attempts);
    }

    // Fix responses, so now we'll progress towards the session being abandoned
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    for i in 0..2 {
        test_harness
            .respond_to_countersigning_workflow_signal()
            .await;

        let summary = test_harness.expect_session_in_unknown_state();
        assert_eq!(i + 1, summary.attempts);
    }

    // Trying again should now abandon the session
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_abandoned_signal().await;
    test_harness.expect_session_removed(request).await;

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn recover_and_complete_after_resolution_failures() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, Some(3)).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Now we're in the accepted state and have committed but haven't received any signatures yet.
    test_harness.expect_session_accepted();

    // Have Bob's authorities return an error
    test_harness.reconfigure_network({
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity()
                .returning(move |_, _, _| Err(HolochainP2pError::Other("test".into())));

            net
        }
    });

    // Fail to progress
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;
    let summary = test_harness.expect_session_in_unknown_state();
    assert_eq!(0, summary.attempts);

    // Fix responses, so now we'll progress towards the session being abandoned
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;
    let summary = test_harness.expect_session_in_unknown_state();
    assert_eq!(1, summary.attempts);

    // Now have Bob showing activity with a bad signature
    let activity_response = bob
        .complete_session_activity_response(
            &session_data,
            entry.clone(),
            &entry_hash,
            test_harness.keystore.clone(),
            false,
        )
        .await;
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net.expect_chc().return_once(|| None);

            net.expect_publish_countersign()
                .return_once(|_, _, _| Ok(()));

            net
        }
    });

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;
    let summary = test_harness.expect_session_in_unknown_state();
    assert_eq!(2, summary.attempts);

    // Now have Bob showing valid activity
    let activity_response = bob
        .complete_session_activity_response(
            &session_data,
            entry.clone(),
            &entry_hash,
            test_harness.keystore.clone(),
            true,
        )
        .await;
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net.expect_chc().return_once(|| None);

            net.expect_publish_countersign()
                .return_once(|_, _, _| Ok(()));

            net
        }
    });

    // Trying again should complete the session
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_success_signal().await;
    test_harness.expect_publish_and_integrate();

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
    test_harness.expect_scheduling_complete();
}

#[tokio::test(flavor = "multi_thread")]
async fn retry_on_network_errors_even_with_no_retries_configured() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash, None).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Now we're in the accepted state and have committed but haven't received any signatures yet.
    test_harness.expect_session_accepted();

    // Have Bob's authorities return an error
    test_harness.reconfigure_network({
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity()
                .returning(move |_, _, _| Err(HolochainP2pError::Other("test".into())));

            net
        }
    });

    for _ in 0..10 {
        test_harness
            .respond_to_countersigning_workflow_signal()
            .await;

        test_harness.expect_session_in_unknown_state();
    }

    // And on and on and on...
    test_harness.expect_session_in_unknown_state();
}

struct TestHarness {
    dna_hash: DnaHash,
    test_space: TestSpace,
    workspace: Arc<CountersigningWorkspace>,
    network: Arc<MockHolochainP2pDnaT>,
    signal_tx: Sender<Signal>,
    signal_rx: Receiver<Signal>,
    keystore: MetaLairClient,
    author: AgentPubKey,
    countersigning_tx: TriggerSender,
    countersigning_rx: TriggerReceiver,
    integration_tx: TriggerSender,
    integration_rx: TriggerReceiver,
    publish_tx: TriggerSender,
    publish_rx: TriggerReceiver,
    remote_agents: usize,
}

/// Test driver implementation
impl TestHarness {
    async fn new(dna_hash: DnaHash, retry_limit: Option<usize>) -> Self {
        let test_space = TestSpace::new(dna_hash.clone());
        let network = MockHolochainP2pDnaT::new();
        let signal = tokio::sync::broadcast::channel::<Signal>(1);
        let keystore = holochain_keystore::test_keystore();
        let author = keystore.new_sign_keypair_random().await.unwrap();
        let countersigning_trigger = TriggerSender::new();
        let integration_trigger = TriggerSender::new();
        let publish_trigger = TriggerSender::new();

        source_chain::genesis(
            test_space
                .space
                .get_or_create_authored_db(author.clone())
                .unwrap(),
            test_space.space.dht_db.clone(),
            &DhtDbQueryCache::new(test_space.space.dht_db.clone().into()),
            keystore.clone(),
            dna_hash.clone(),
            author.clone(),
            None,
            None,
        )
        .await
        .unwrap();

        let cell_id = CellId::new(dna_hash.clone(), author.clone());
        let workspace = Arc::new(CountersigningWorkspace::new(
            Duration::from_secs(1),
            retry_limit,
        ));
        test_space
            .space
            .countersigning_workspaces
            .lock()
            .insert(cell_id.clone(), workspace.clone());

        Self {
            dna_hash,
            test_space,
            workspace,
            network: Arc::new(network),
            signal_tx: signal.0,
            signal_rx: signal.1,
            keystore,
            author,
            countersigning_tx: countersigning_trigger.0,
            countersigning_rx: countersigning_trigger.1,
            integration_tx: integration_trigger.0,
            integration_rx: integration_trigger.1,
            publish_tx: publish_trigger.0,
            publish_rx: publish_trigger.1,
            remote_agents: 0,
        }
    }

    async fn new_remote_agent(&mut self) -> RemoteAgent {
        self.remote_agents += 1;
        RemoteAgent {
            agent: self.keystore.new_sign_keypair_random().await.unwrap(),
            agent_index: self.remote_agents,
            chain_head: ChainHead {
                action_seq: 32,
                hash: fixt!(ActionHash),
            },
        }
    }

    fn reconfigure_network(
        &mut self,
        apply: impl Fn(MockHolochainP2pDnaT) -> MockHolochainP2pDnaT,
    ) {
        let network = apply(MockHolochainP2pDnaT::new());
        self.network = Arc::new(network);
    }

    async fn accept_countersigning_request(
        &self,
        request: PreflightRequest,
    ) -> WorkflowResult<PreflightRequestAcceptance> {
        accept_countersigning_request(
            self.test_space.space.clone(),
            self.keystore.clone(),
            self.author.clone(),
            request,
            self.countersigning_tx.clone(),
        )
        .await
    }

    async fn respond_to_countersigning_workflow_signal(&mut self) {
        tokio::time::timeout(Duration::from_secs(5), self.countersigning_rx.listen())
            .await
            .expect("Didn't receive a trigger in time")
            .unwrap();

        let outcome = countersigning_workflow(
            self.test_space.space.clone(),
            self.workspace.clone(),
            self.network.clone(),
            self.keystore.clone(),
            CellId::new(self.dna_hash.clone(), self.author.clone()),
            self.signal_tx.clone(),
            self.countersigning_tx.clone(),
            self.integration_tx.clone(),
            self.publish_tx.clone(),
        )
        .await
        .unwrap();

        assert_eq!(WorkComplete::Complete, outcome);
    }

    async fn unlock_chain(&self) {
        let authored = self
            .test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap();
        authored
            .write_async({
                let author = self.author.clone();
                move |txn| unlock_chain(txn, &author)
            })
            .await
            .unwrap();
    }

    fn clear_workspace_session(&self) {
        self.workspace
            .inner
            .share_mut(|w, _| {
                w.session = None;
                Ok(())
            })
            .unwrap();
    }

    fn build_session_data(
        &self,
        request: PreflightRequest,
        acceptances: Vec<PreflightRequestAcceptance>,
    ) -> (CounterSigningSessionData, Entry, EntryHash) {
        let session_data = CounterSigningSessionData::try_new(
            request,
            acceptances
                .into_iter()
                .filter_map(|a| match a {
                    PreflightRequestAcceptance::Accepted(a) => Some((a.agent_state, a.signature)),
                    _ => None,
                })
                .collect(),
            vec![],
        )
        .unwrap();

        let entry = Entry::CounterSign(Box::new(session_data.clone()), fixt!(AppEntryBytes));
        let entry_hash = EntryHash::with_data_sync(&entry);

        (session_data, entry, entry_hash)
    }

    async fn commit_countersigning_entry(
        &self,
        session_data: &CounterSigningSessionData,
        entry: Entry,
        entry_hash: EntryHash,
    ) -> SignedAction {
        let my_action = Action::from_countersigning_data(
            entry_hash.clone(),
            session_data,
            self.author.clone(),
            weigh_placeholder(),
        )
        .unwrap();
        let hashed = ActionHashed::from_content_sync(my_action.clone());
        let sah = SignedActionHashed::sign(&self.keystore, hashed)
            .await
            .unwrap();

        let signed = SignedAction::from(sah.clone());

        let store_entry_op = ChainOp::StoreEntry(
            fixt!(Signature),
            my_action.clone().try_into().unwrap(),
            entry.clone(),
        );
        let dht_op = DhtOp::ChainOp(Box::new(store_entry_op));
        let dht_op = DhtOpHashed::from_content_sync(dht_op);

        self.test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap()
            .write_async(move |txn| -> StateMutationResult<()> {
                insert_action(txn, &sah)?;
                insert_entry(txn, &entry_hash, &entry)?;
                insert_op_dht(txn, &dht_op)?;
                set_withhold_publish(txn, &dht_op.hash)?;

                Ok(())
            })
            .await
            .unwrap();

        signed
    }

    async fn read_chain_head_hash(&self) -> HeadInfo {
        let authored = self
            .test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap();
        let chain_head = authored
            .read_async({
                let author = self.author.clone();
                move |txn| chain_head_db(txn, Arc::new(author))
            })
            .await
            .unwrap();

        chain_head.unwrap()
    }

    async fn try_remove_countersigning_entry(
        &self,
        action_hash: ActionHash,
    ) -> StateMutationResult<()> {
        let authored = self
            .test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap();
        authored
            .write_async({
                move |txn| {
                    let blob: Vec<u8> = txn.query_row(
                        "SELECT blob FROM Action WHERE hash = ?",
                        [action_hash],
                        |r| r.get(0),
                    )?;
                    remove_countersigning_session(
                        txn,
                        from_blob::<SignedAction>(blob)?.action().clone(),
                        fixt!(EntryHash),
                    )
                }
            })
            .await
    }
}

/// Assertion query implementation
impl TestHarness {
    fn expect_empty_workspace(&self) {
        let no_session = self
            .workspace
            .inner
            .share_ref(|w| Ok(w.session.is_none()))
            .unwrap();

        assert!(no_session);
    }

    fn expect_session_accepted(&self) -> PreflightRequest {
        let maybe_found = self
            .workspace
            .inner
            .share_ref(|w| Ok(w.session.clone()))
            .unwrap();

        assert!(maybe_found.is_some());

        match maybe_found.unwrap() {
            CountersigningSessionState::Accepted(preflight_request) => preflight_request,
            _ => panic!("Session not in accepted state"),
        }
    }

    fn expect_session_in_unknown_state(&self) -> SessionResolutionSummary {
        let maybe_found = self
            .workspace
            .inner
            .share_ref(|w| Ok(w.session.clone()))
            .unwrap();

        assert!(maybe_found.is_some());

        match maybe_found {
            Some(CountersigningSessionState::Unknown { resolution, .. }) => resolution,
            state => panic!("Session not in unknown state: {:?}", state),
        }
    }

    fn expect_session_in_signatures_collected(&self) -> Option<SessionResolutionSummary> {
        let maybe_found = self
            .workspace
            .inner
            .share_ref(|w| Ok(w.session.clone()))
            .unwrap();

        assert!(maybe_found.is_some());

        match maybe_found {
            Some(CountersigningSessionState::SignaturesCollected {
                resolution,
                signature_bundles,
                ..
            }) => {
                // Signatures should always have been consumed by the time we are doing an assertion
                assert!(signature_bundles.is_empty());

                resolution
            }
            state => panic!("Session not in unknown state: {:?}", state),
        }
    }

    async fn expect_chain_locked(&self) {
        let authored = self
            .test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap();
        let lock = authored
            .read_async({
                let author = self.author.clone();
                move |txn| get_chain_lock(txn, &author)
            })
            .await
            .unwrap();

        assert!(lock.is_some());
    }

    pub async fn expect_chain_unlocked(&self) {
        let authored = self
            .test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap();
        let lock = authored
            .read_async({
                let author = self.author.clone();
                move |txn| get_chain_lock(txn, &author)
            })
            .await
            .unwrap();

        assert!(lock.is_none());
    }

    pub async fn expect_session_removed(&self, preflight_request: PreflightRequest) {
        let authored = self
            .test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap();

        let session = authored
            .read_async({
                let author = self.author.clone();
                move |txn| current_countersigning_session(txn, Arc::new(author))
            })
            .await
            .unwrap();

        match session {
            None => {
                // Good, no session on the chain top
            }
            Some((_, _, session)) => {
                // If there is a session, it must not match!
                assert_ne!(
                    session.preflight_request.fingerprint(),
                    preflight_request.fingerprint()
                );
            }
        }
    }

    pub async fn expect_abandoned_signal(&mut self) {
        let signal = tokio::time::timeout(std::time::Duration::from_secs(1), self.signal_rx.recv())
            .await
            .expect("Didn't receive a signal in time")
            .unwrap();

        assert_matches!(
            signal,
            Signal::System(SystemSignal::AbandonedCountersigning(_))
        );
    }

    pub async fn expect_success_signal(&mut self) {
        let signal = tokio::time::timeout(std::time::Duration::from_secs(1), self.signal_rx.recv())
            .await
            .expect("Didn't receive a signal in time")
            .unwrap();

        assert_matches!(
            signal,
            Signal::System(SystemSignal::SuccessfulCountersigning(_))
        );
    }

    pub fn expect_publish_and_integrate(&mut self) {
        self.integration_rx.try_recv().unwrap();
        self.publish_rx.try_recv().unwrap();
    }

    fn expect_no_pending_signals(&mut self) {
        let signal = self.signal_rx.try_recv().ok();
        assert!(signal.is_none());

        let trigger = self.countersigning_rx.try_recv();
        assert!(trigger.is_none());

        let trigger = self.integration_rx.try_recv();
        assert!(trigger.is_none());

        let trigger = self.publish_rx.try_recv();
        assert!(trigger.is_none());
    }

    fn expect_scheduling_complete(&self) {
        self.workspace
            .inner
            .share_ref(|inner| {
                match &inner.next_trigger {
                    Some(next_trigger) => {
                        assert!(next_trigger.trigger_at < Timestamp::now());
                    }
                    None => {}
                }

                Ok(())
            })
            .unwrap();
    }
}

struct RemoteAgent {
    agent: AgentPubKey,
    agent_index: usize,
    chain_head: ChainHead,
}

impl RemoteAgent {
    async fn accept_preflight_request(
        &self,
        request: PreflightRequest,
        keystore: MetaLairClient,
    ) -> PreflightRequestAcceptance {
        let agent_state = CounterSigningAgentState::new(
            self.agent_index as u8,
            self.chain_head.hash.clone(),
            self.chain_head.action_seq,
        );
        let response_data =
            PreflightResponse::encode_fields_for_signature(&request, &agent_state).unwrap();
        let signature = keystore
            .sign(self.agent.clone(), response_data.into())
            .await
            .unwrap();

        PreflightRequestAcceptance::Accepted(
            PreflightResponse::try_new(request.clone(), agent_state, signature).unwrap(),
        )
    }

    async fn produce_signature(
        &self,
        session_data: &CounterSigningSessionData,
        entry_hash: &EntryHash,
        keystore: MetaLairClient,
    ) -> SignedAction {
        let action = Action::from_countersigning_data(
            entry_hash.clone(),
            session_data,
            self.agent.clone(),
            weigh_placeholder(),
        )
        .unwrap();

        let hashed = ActionHashed::from_content_sync(action.clone());
        let sah = SignedActionHashed::sign(&keystore, hashed).await.unwrap();

        SignedAction::from(sah)
    }

    fn no_activity_response(&self) -> AgentActivityResponse {
        AgentActivityResponse {
            agent: self.agent.clone(),
            valid_activity: ChainItems::Full(vec![]),
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Valid(self.chain_head.clone()),
            highest_observed: None,
            warrants: vec![],
        }
    }

    fn other_activity_response(&self) -> AgentActivityResponse {
        let action = Action::Create(fixt!(Create));

        AgentActivityResponse {
            agent: self.agent.clone(),
            valid_activity: ChainItems::Full(vec![Record::new(
                SignedActionHashed::new_unchecked(action, fixt!(Signature)),
                Some(fixt!(Entry)),
            )]),
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Valid(ChainHead {
                action_seq: self.chain_head.action_seq + 1,
                hash: fixt!(ActionHash), // Won't match the action activity hash, doesn't matter
            }),
            highest_observed: None,
            warrants: vec![],
        }
    }

    async fn complete_session_activity_response(
        &self,
        session_data: &CounterSigningSessionData,
        entry: Entry,
        entry_hash: &EntryHash,
        keystore: MetaLairClient,
        valid_signature: bool,
    ) -> AgentActivityResponse {
        let signed_action = self
            .produce_signature(session_data, entry_hash, keystore)
            .await;
        let signature = if valid_signature {
            signed_action.signature().clone()
        } else {
            fixt!(Signature)
        };

        AgentActivityResponse {
            agent: self.agent.clone(),
            valid_activity: ChainItems::Full(vec![Record::new(
                SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(signed_action.into_data()),
                    signature,
                ),
                Some(entry),
            )]),
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Valid(ChainHead {
                action_seq: self.chain_head.action_seq + 1,
                hash: fixt!(ActionHash), // Won't match the action activity hash, doesn't matter
            }),
            highest_observed: None,
            warrants: vec![],
        }
    }
}

fn test_preflight_request(
    test_harness: &TestHarness,
    duration: std::time::Duration,
    other: &RemoteAgent,
) -> PreflightRequest {
    PreflightRequest::try_new(
        fixt!(EntryHash),
        vec![
            (test_harness.author.clone(), vec![]),
            (other.agent.clone(), vec![]),
        ],
        vec![],
        0,
        false,
        CounterSigningSessionTimes {
            start: Timestamp::now(),
            end: Timestamp::now().add(duration).unwrap(),
        },
        ActionBase::Create(CreateBase::new(fixt!(EntryType))),
        PreflightBytes(vec![]),
    )
    .unwrap()
}
