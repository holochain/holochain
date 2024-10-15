use hdk::prelude::{
    PreflightRequest, PreflightRequestAcceptance, Signature, SignedActionHashed, Timestamp,
};
use holo_hash::{ActionHash, EntryHash};
use holochain::conductor::api::error::{ConductorApiError, ConductorApiResult};
use holochain::conductor::CellError;
use holochain::core::workflow::WorkflowError;
use holochain::prelude::CountersigningSessionState;
use holochain::sweettest::{
    await_consistency, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
};
use holochain_nonce::fresh_nonce;
use holochain_state::prelude::{IncompleteCommitReason, SourceChainError};
use holochain_types::app::DisabledAppReason;
use holochain_types::prelude::Signal;
use holochain_types::signal::SystemSignal;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::countersigning::Role;
use matches::assert_matches;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::Receiver;

mod session_interaction_over_websocket;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "windows", ignore = "flaky")]
async fn listen_for_countersigning_completion() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob]).await.unwrap();

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let (_, _): (ActionHash, EntryHash) = conductors[1]
        .call_fallible(
            &bob_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let alice_rx = conductors[0].subscribe_to_app_signals("app".into());
    let bob_rx = conductors[1].subscribe_to_app_signals("app".into());

    wait_for_completion(alice_rx, preflight_request.app_entry_hash.clone()).await;
    wait_for_completion(bob_rx, preflight_request.app_entry_hash).await;
}

// Regression test to check that it's possible to continue a countersigning session following
// a failed commit that required dependencies that couldn't be fetched.
#[tokio::test(flavor = "multi_thread")]
async fn retry_countersigning_commit_on_missing_deps() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;
    for conductor in conductors.iter_mut() {
        conductor.shutdown().await;
    }
    // Allow bootstrapping and network comms so that peers can find each other, and exchange DPKI info,
    // but before creating any data, disable publish and recent gossip.
    // Now the only way peers can get data is through get requests!
    for conductor in conductors.iter_mut() {
        conductor.update_config(|c| SweetConductorConfig::from(c).historical_only().into());
        conductor.startup().await;
    }
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    // Make sure both peers can see each other so that we know get requests should succeed when
    // fetching agent activity
    conductors[0]
        .wait_for_peer_visible([bob.agent_pubkey().clone()], None, Duration::from_secs(30))
        .await
        .unwrap();
    conductors[1]
        .wait_for_peer_visible(
            [alice.agent_pubkey().clone()],
            None,
            Duration::from_secs(30),
        )
        .await
        .unwrap();

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    // Take Bob's app offline so that Alice can't get his activity
    conductors[1]
        .disable_app("app".into(), DisabledAppReason::User)
        .await
        .unwrap();

    // Alice shouldn't be able to commit yet, because she doesn't have Bob's activity
    let result: ConductorApiResult<(ActionHash, EntryHash)> = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await;
    match result {
        Ok(_) => {
            panic!("Expected commit to fail due to missing dependencies");
        }
        Err(ConductorApiError::CellError(CellError::WorkflowError(e))) => {
            match *e {
                WorkflowError::SourceChainError(SourceChainError::IncompleteCommit(
                    IncompleteCommitReason::DepMissingFromDht(_),
                )) => {
                    // Expected
                }
                _ => {
                    panic!("Expected IncompleteCommit error, got: {:?}", e);
                }
            }
        }
        _ => {
            panic!("Expected CellError::WorkflowError, got: {:?}", result);
        }
    }

    // Bring Bob's app back online
    conductors[1].enable_app("app".into()).await.unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;

    // Bob should be able to get Alice's chain head when we commit, so let's do that.
    let (_, _): (ActionHash, EntryHash) = conductors[1]
        .call_fallible(
            &bob_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    // Now that Bob is available again, Alice should also be able to get his chain head and complete
    // her commit
    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    // Listen for the session to complete, which it should in spite of the error that
    // Alice had initially.

    let alice_rx = conductors[0].subscribe_to_app_signals("app".into());
    let bob_rx = conductors[1].subscribe_to_app_signals("app".into());

    wait_for_completion(alice_rx, preflight_request.app_entry_hash.clone()).await;
    wait_for_completion(bob_rx, preflight_request.app_entry_hash).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn alice_can_recover_when_bob_abandons_a_countersigning_session() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true).tune_conductor(|c| {
        c.countersigning_resolution_retry_delay = Some(Duration::from_secs(3));
    });
    let mut conductors = SweetConductorBatch::from_config_rendezvous(3, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];
    let carol = &cells[2];

    // Subscribe early in the test to avoid missing signals later
    let alice_signal_rx = conductors[0].subscribe_to_app_signals("app".into());
    let bob_signal_rx = conductors[1].subscribe_to_app_signals("app".into());

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob, carol])
        .await
        .unwrap();

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    // Alice doesn't realise that Bob is a very bad man, and goes ahead and commits
    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    // Bob does not commit, and instead abandons the session

    // Bob's session should time out and be abandoned
    wait_for_abandoned(bob_signal_rx, preflight_request.app_entry_hash.clone()).await;

    // Alice session should also get abandoned
    wait_for_abandoned(alice_signal_rx, preflight_request.app_entry_hash.clone()).await;

    // Alice will now be allowed to commit other entries
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();

    // Everyone's DHT should sync
    await_consistency(60, [alice, bob, carol]).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn alice_can_recover_from_a_session_timeout() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true).tune_conductor(|c| {
        c.countersigning_resolution_retry_delay = Some(Duration::from_secs(3));
    });
    let mut conductors = SweetConductorBatch::from_config_rendezvous(3, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];
    let carol = &cells[2];

    // Make sure the conductors are gossiping before creating posts
    conductors[0]
        .require_initial_gossip_activity_for_cell(alice, 3, Duration::from_secs(30))
        .await
        .unwrap();

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob, carol])
        .await
        .unwrap();

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    // Alice makes her commit, but before Bob can do the same, disaster strikes...
    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    // Give Alice some time to publish her session.
    // TODO Does Holochain not block on this network operation?
    tokio::time::sleep(Duration::from_secs(3)).await;

    conductors[0].shutdown().await;

    // Bob can't know what has happened to Alice and makes his commit
    let (_, _): (ActionHash, EntryHash) = conductors[1]
        .call_fallible(
            &bob_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    tracing::info!("Waiting for Bob completion");

    // Luckily Carol is still online, she should get both Alice and Bob's signatures.
    // She will send these to Bob, and he can complete his session.
    // Bob's session should time out and be abandoned
    wait_for_completion(
        conductors[1].subscribe_to_app_signals("app".into()),
        preflight_request.app_entry_hash.clone(),
    )
    .await;

    // Alice comes back online, and should be able to recover her session. It will take finding
    // her session in her source chain and then trying to build a signature bundle from the network.
    conductors[0].startup().await;

    tracing::warn!("Alice is back online");

    // Alice session should now get completed
    wait_for_completion(
        conductors[0].subscribe_to_app_signals("app".into()),
        preflight_request.app_entry_hash.clone(),
    )
    .await;

    // Alice will now be allowed to commit other entries
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();

    // Everyone's DHT should sync
    await_consistency(60, [alice, bob, carol]).await.unwrap();
}

#[cfg(feature = "chc")]
#[tokio::test(flavor = "multi_thread")]
async fn complete_session_with_chc_enabled() {
    holochain_trace::test_run();

    let mut config = SweetConductorConfig::rendezvous(true);
    config.chc_url = Some(url2::Url2::parse(
        holochain::conductor::chc::CHC_LOCAL_MAGIC_URL,
    ));

    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    conductors[0]
        .require_initial_gossip_activity_for_cell(alice, 2, Duration::from_secs(30))
        .await
        .unwrap();

    let alice_chc = conductors[0].get_chc(alice.cell_id()).unwrap();

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob]).await.unwrap();

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    let before_chain = get_all_records(&alice_chc).await.unwrap();

    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let between_chain = get_all_records(&alice_chc).await.unwrap();

    let (_, _): (ActionHash, EntryHash) = conductors[1]
        .call_fallible(
            &bob_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let alice_rx = conductors[0].subscribe_to_app_signals("app".into());
    let bob_rx = conductors[1].subscribe_to_app_signals("app".into());

    wait_for_completion(alice_rx, preflight_request.app_entry_hash.clone()).await;
    wait_for_completion(bob_rx, preflight_request.app_entry_hash).await;

    let after_chain = get_all_records(&alice_chc).await.unwrap();

    // Should not appear in the CHC after commit, must wait for publish
    assert_eq!(before_chain.len(), between_chain.len());

    // Should appear in the CHC after publish
    assert_eq!(before_chain.len() + 1, after_chain.len());
}

#[cfg(feature = "chc")]
#[tokio::test(flavor = "multi_thread")]
async fn session_rollback_with_chc_enabled() {
    holochain_trace::test_run();

    let mut config = SweetConductorConfig::rendezvous(true);
    config.chc_url = Some(url2::Url2::parse(
        holochain::conductor::chc::CHC_LOCAL_MAGIC_URL,
    ));

    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    let alice_chc = conductors[0].get_chc(alice.cell_id()).unwrap();

    // Make sure the conductors are gossiping before continuing
    conductors[0]
        .require_initial_gossip_activity_for_cell(alice, 2, Duration::from_secs(30))
        .await
        .unwrap();

    // Subscribe early in the test to avoid missing signals later
    let alice_signal_rx = conductors[0].subscribe_to_app_signals("app".into());
    let bob_signal_rx = conductors[1].subscribe_to_app_signals("app".into());

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob]).await.unwrap();

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    let before_chain = get_all_records(&alice_chc).await.unwrap();

    // Alice goes ahead and commits. This must not go to the CHC yet!
    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let between_chain = get_all_records(&alice_chc).await.unwrap();

    // Bob does not commit, and instead abandons the session

    // Bob's session should time out and be abandoned
    wait_for_abandoned(bob_signal_rx, preflight_request.app_entry_hash.clone()).await;

    // Alice session should also get abandoned
    wait_for_abandoned(alice_signal_rx, preflight_request.app_entry_hash.clone()).await;

    let after_chain = get_all_records(&alice_chc).await.unwrap();

    // Alice will now be allowed to commit other entries
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();

    let future_chain = get_all_records(&alice_chc).await.unwrap();

    // Should not appear in the CHC after commit, must wait for publish
    assert_eq!(before_chain.len(), between_chain.len());
    // Publish didn't happen, so should not have been added to the CHC
    assert_eq!(between_chain.len(), after_chain.len());
    // Now we've written something new to the chain, so the CHC should have been updated
    assert_eq!(before_chain.len() + 1, future_chain.len());

    // Everyone's DHT should sync
    await_consistency(60, [alice, bob]).await.unwrap();
}

#[cfg(feature = "chc")]
#[tokio::test(flavor = "multi_thread")]
async fn multiple_agents_on_same_conductor_with_chc_enabled() {
    holochain_trace::test_run();

    let mut config = SweetConductorConfig::rendezvous(true);
    config.chc_url = Some(url2::Url2::parse(
        holochain::conductor::chc::CHC_LOCAL_MAGIC_URL,
    ));

    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna.clone()]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    // Carol installs the same DNA on the conductor that Alice is using
    let carol_app = conductors[0].setup_app("app2", &[dna]).await.unwrap();
    let carol = &carol_app.cells()[0];

    let alice_chc = conductors[0].get_chc(alice.cell_id()).unwrap();
    let carol_chc = conductors[0].get_chc(carol.cell_id()).unwrap();

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();
    let carol_zome = carol.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&carol_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob, carol])
        .await
        .unwrap();

    // Set up the session and accept it for both Alice and Bob
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    let before_chain = get_all_records(&alice_chc).await.unwrap();

    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let between_chain = get_all_records(&alice_chc).await.unwrap();

    let (_, _): (ActionHash, EntryHash) = conductors[1]
        .call_fallible(
            &bob_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let alice_rx = conductors[0].subscribe_to_app_signals("app".into());
    let bob_rx = conductors[1].subscribe_to_app_signals("app".into());

    wait_for_completion(alice_rx, preflight_request.app_entry_hash.clone()).await;
    wait_for_completion(bob_rx, preflight_request.app_entry_hash).await;

    let after_chain = get_all_records(&alice_chc).await.unwrap();

    // Should not appear in the CHC after commit, must wait for publish
    assert_eq!(before_chain.len(), between_chain.len());

    // Should appear in the CHC after publish
    assert_eq!(before_chain.len() + 1, after_chain.len());

    await_consistency(30, vec![alice, bob, carol])
        .await
        .unwrap();

    // Now Carol should be able to do a session with Bob

    // Set up the session and accept it for both Carol and Bob
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &carol_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (carol.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let carol_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &carol_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let carol_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = carol_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    let before_chain = get_all_records(&carol_chc).await.unwrap();

    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &carol_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![carol_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let between_chain = get_all_records(&carol_chc).await.unwrap();

    let (_, _): (ActionHash, EntryHash) = conductors[1]
        .call_fallible(
            &bob_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![carol_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let carol_rx = conductors[0].subscribe_to_app_signals("app2".into());
    let bob_rx = conductors[1].subscribe_to_app_signals("app".into());

    wait_for_completion(carol_rx, preflight_request.app_entry_hash.clone()).await;
    wait_for_completion(bob_rx, preflight_request.app_entry_hash).await;

    let after_chain = get_all_records(&carol_chc).await.unwrap();

    // Should not appear in the CHC after commit, must wait for publish
    assert_eq!(before_chain.len(), between_chain.len());

    // Should appear in the CHC after publish
    assert_eq!(before_chain.len() + 1, after_chain.len());
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "windows", ignore = "flaky")]
async fn chc_should_respect_chain_lock() {
    holochain_trace::test_run();

    let mut config = SweetConductorConfig::rendezvous(true);
    config.chc_url = Some(url2::Url2::parse(
        holochain::conductor::chc::CHC_LOCAL_MAGIC_URL,
    ));
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob]).await.unwrap();

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    // After accept, Alice's chain is locked and she should not be able to commit
    conductors[0]
        .call_fallible::<_, ActionHash>(&alice_zome, "create_a_thing", ())
        .await
        .unwrap_err();

    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    // The same is true after committing the countersigning entry, the chain is still locked!
    conductors[0]
        .call_fallible::<_, ActionHash>(&alice_zome, "create_a_thing", ())
        .await
        .unwrap_err();

    let (_, _): (ActionHash, EntryHash) = conductors[1]
        .call_fallible(
            &bob_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    let alice_rx = conductors[0].subscribe_to_app_signals("app".into());
    let bob_rx = conductors[1].subscribe_to_app_signals("app".into());

    wait_for_completion(alice_rx, preflight_request.app_entry_hash.clone()).await;
    wait_for_completion(bob_rx, preflight_request.app_entry_hash).await;

    // Now a commit should succeed because the session has finished and we shouldn't be behind the CHC.
    conductors[0]
        .call_fallible::<_, ActionHash>(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "windows", ignore = "flaky")]
async fn should_be_able_to_schedule_functions_during_session() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    // Make sure the conductors are gossiping before creating posts
    conductors[0]
        .require_initial_gossip_activity_for_cell(alice, 2, Duration::from_secs(30))
        .await
        .unwrap();

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob]).await.unwrap();

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();

    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    assert_matches!(alice_acceptance, PreflightRequestAcceptance::Accepted(_));

    conductors[0]
        .call_fallible::<_, ()>(&alice_zome, "schedule_signal", ())
        .await
        .unwrap();

    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    assert_matches!(bob_acceptance, PreflightRequestAcceptance::Accepted(_));

    conductors[1]
        .call_fallible::<_, ()>(&bob_zome, "schedule_signal", ())
        .await
        .unwrap();

    let sig = conductors[0]
        .subscribe_to_app_signals("app".into())
        .recv()
        .await
        .unwrap();
    match sig {
        Signal::App { signal, .. } => {
            let msg = signal.into_inner().decode::<String>().unwrap();
            assert_eq!("scheduled hello", msg);
        }
        _ => panic!("Expected App signal"),
    }

    let sig = conductors[1]
        .subscribe_to_app_signals("app".into())
        .recv()
        .await
        .unwrap();
    match sig {
        Signal::App { signal, .. } => {
            let msg = signal.into_inner().decode::<String>().unwrap();
            assert_eq!("scheduled hello", msg);
        }
        _ => panic!("Expected App signal"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn alice_can_force_abandon_session_when_automatic_resolution_has_failed_after_shutdown() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|c| {
            c.countersigning_resolution_retry_delay = Some(Duration::from_secs(3));
        })
        .tune(|params| {
            // Incredible, but true: set the timeout for a network
            params.tx2_implicit_timeout_ms = 3_000;
        });
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let app_id = "app";
    let apps = conductors.setup_app(app_id, &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob]).await.unwrap();

    // Set up the session and accept it for both agents.
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    // Alice makes her commit and shuts down.
    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();
    conductors[0].shutdown().await;

    // Alice comes back online.
    conductors[0].startup().await;

    println!("here we are");

    // Wait until Alice's session has been attempted to be resolved.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let state = conductors[0]
            .raw_handle()
            .get_countersigning_session_state(alice.cell_id())
            .await
            .unwrap();
            if matches!(
                state,
                Some(CountersigningSessionState::Unknown { resolution, .. }) if resolution.attempts >= 1
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    })
    .await
    .unwrap();

    println!("here we are again");

    let mut alice_app_signal_rx = conductors[0].subscribe_to_app_signals(app_id.to_string());
    let mut bob_app_signal_rx = conductors[1].subscribe_to_app_signals(app_id.to_string());

    // Alice abandons the session.
    conductors[0]
        .abandon_countersigning_session(alice.cell_id())
        .await
        .unwrap();

    // Await countersigning session abandoned signal for Alice.
    match alice_app_signal_rx.recv().await.unwrap() {
        Signal::System(SystemSignal::AbandonedCountersigning(entry_hash)) => {
            assert_eq!(entry_hash, preflight_request.app_entry_hash.clone());
        }
        _ => panic!("Expected System signal"),
    }
    // Alice's session should be gone from memory.
    let alice_state = conductors[0]
        .raw_handle()
        .get_countersigning_session_state(alice.cell_id())
        .await
        .unwrap();
    assert_matches!(alice_state, None);

    // Await countersigning session abandoned signal for Bob.
    match bob_app_signal_rx.recv().await.unwrap() {
        Signal::System(SystemSignal::AbandonedCountersigning(entry_hash)) => {
            assert_eq!(entry_hash, preflight_request.app_entry_hash.clone());
        }
        _ => panic!("Expected System signal"),
    }
    // Bob's session should be gone from memory.
    let bob_state = conductors[1]
        .raw_handle()
        .get_countersigning_session_state(bob.cell_id())
        .await
        .unwrap();
    assert_matches!(bob_state, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn alice_can_force_publish_session_when_automatic_resolution_has_failed_after_shutdown() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|c| {
            c.countersigning_resolution_retry_delay = Some(Duration::from_secs(3));
        })
        .tune(|params| {
            // Incredible, but true: set the timeout for a network
            params.tx2_implicit_timeout_ms = 3_000;
        });
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let app_id = "app";
    let apps = conductors.setup_app(app_id, &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice = &cells[0];
    let bob = &cells[1];

    // Need an initialised source chain for countersigning, so commit anything
    let alice_zome = alice.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();
    let bob_zome = bob.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[1]
        .call_fallible(&bob_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob]).await.unwrap();

    // Set up the session and accept it for both agents.
    let preflight_request: PreflightRequest = conductors[0]
        .call_fallible(
            &alice_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (alice.agent_pubkey().clone(), vec![Role(0)]),
                (bob.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let alice_acceptance: PreflightRequestAcceptance = conductors[0]
        .call_fallible(
            &alice_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = conductors[1]
        .call_fallible(
            &bob_zome,
            "accept_countersigning_preflight_request",
            preflight_request.clone(),
        )
        .await
        .unwrap();
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    // Alice makes her commit and shuts down.
    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();
    conductors[0].shutdown().await;

    // Bob can't know what has happened to Alice, makes his commit and shuts down.
    let (_, _): (ActionHash, EntryHash) = conductors[1]
        .call_fallible(
            &bob_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();
    conductors[1].shutdown().await;

    // Alice comes back online.
    conductors[0].startup().await;

    // Bob comes back online too.
    conductors[1].startup().await;

    // Wait until Alice's session has been attempted to be resolved.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let state = conductors[0]
            .raw_handle()
            .get_countersigning_session_state(alice.cell_id())
            .await
            .unwrap();
            if matches!(
                state,
                Some(CountersigningSessionState::Unknown { resolution, .. }) if resolution.attempts >= 1
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    })
    .await
    .unwrap();

    let mut alice_app_signal_rx = conductors[0].subscribe_to_app_signals(app_id.to_string());
    let mut bob_app_signal_rx = conductors[1].subscribe_to_app_signals(app_id.to_string());

    // Alice publishes the session.
    conductors[0]
        .publish_countersigning_session(alice.cell_id())
        .await
        .unwrap();

    // Await countersigning success signal for Alice.
    match alice_app_signal_rx.recv().await.unwrap() {
        Signal::System(SystemSignal::SuccessfulCountersigning(entry_hash)) => {
            assert_eq!(entry_hash, preflight_request.app_entry_hash.clone());
        }
        _ => panic!("Expected System signal"),
    }
    // Alice's session should be gone from memory.
    let alice_state = conductors[0]
        .raw_handle()
        .get_countersigning_session_state(alice.cell_id())
        .await
        .unwrap();
    assert_matches!(alice_state, None);

    // Await countersigning success signal for Bob.
    match bob_app_signal_rx.recv().await.unwrap() {
        Signal::System(SystemSignal::SuccessfulCountersigning(entry_hash)) => {
            assert_eq!(entry_hash, preflight_request.app_entry_hash.clone());
        }
        _ => panic!("Expected System signal"),
    }
    // Bob's session should be gone from memory.
    let bob_state = conductors[1]
        .raw_handle()
        .get_countersigning_session_state(bob.cell_id())
        .await
        .unwrap();
    assert_matches!(bob_state, None);
}

async fn wait_for_completion(mut signal_rx: Receiver<Signal>, expected_hash: EntryHash) {
    let signal = tokio::time::timeout(std::time::Duration::from_secs(30), signal_rx.recv())
        .await
        .unwrap()
        .unwrap();
    match signal {
        Signal::System(SystemSignal::SuccessfulCountersigning(hash)) => {
            assert_eq!(expected_hash, hash);
        }
        _ => {
            panic!(
                "Expected SuccessfulCountersigning signal, got: {:?}",
                signal
            );
        }
    }
}

async fn wait_for_abandoned(mut signal_rx: Receiver<Signal>, expected_hash: EntryHash) {
    let signal = tokio::time::timeout(std::time::Duration::from_secs(30), signal_rx.recv())
        .await
        .unwrap()
        .unwrap();
    match signal {
        Signal::System(SystemSignal::AbandonedCountersigning(hash)) => {
            assert_eq!(expected_hash, hash);
        }
        _ => {
            panic!(
                "Expected SuccessfulCountersigning signal, got: {:?}",
                signal
            );
        }
    }
}

#[cfg(feature = "chc")]
async fn get_all_records(
    chc: &holochain_chc::ChcImpl,
) -> holochain_chc::ChcResult<
    Vec<(
        SignedActionHashed,
        Option<(Arc<holochain_chc::EncryptedEntry>, Signature)>,
    )>,
> {
    use holochain_types::fixt::SignatureFixturator;

    chc.get_record_data_request(holochain_chc::GetRecordsRequest {
        payload: holochain_chc::GetRecordsPayload {
            since_hash: None,
            nonce: fresh_nonce(Timestamp::now()).unwrap().0,
        },
        signature: fixt::fixt!(Signature),
    })
    .await
}
