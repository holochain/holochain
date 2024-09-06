use hdk::prelude::{PreflightRequest, PreflightRequestAcceptance};
use holo_hash::{ActionHash, EntryHash};
use holochain::conductor::api::error::{ConductorApiError, ConductorApiResult};
use holochain::conductor::CellError;
use holochain::core::workflow::WorkflowError;
use holochain::prelude::PreflightResponse;
use holochain::sweettest::{
    await_consistency, SweetCell, SweetConductor, SweetConductorBatch, SweetConductorConfig,
    SweetDnaFile,
};
use holochain_state::prelude::{IncompleteCommitReason, SourceChainError};
use holochain_types::app::DisabledAppReason;
use holochain_types::prelude::Signal;
use holochain_types::signal::SystemSignal;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::countersigning::Role;
use holochain_zome_types::prelude::{
    ActivityRequest, AgentActivity, ChainQueryFilter, GetAgentActivityInput,
};
use std::time::Duration;
use tokio::sync::broadcast::Receiver;

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

    // Need chain head for each other, so get agent activity before starting a session
    let _: AgentActivity = conductors[0]
        .call_fallible(
            &alice_zome,
            "get_agent_activity",
            GetAgentActivityInput {
                agent_pubkey: bob.agent_pubkey().clone(),
                chain_query_filter: ChainQueryFilter::new(),
                activity_request: ActivityRequest::Full,
            },
        )
        .await
        .unwrap();
    let _: AgentActivity = conductors[1]
        .call_fallible(
            &bob_zome,
            "get_agent_activity",
            GetAgentActivityInput {
                agent_pubkey: alice.agent_pubkey().clone(),
                chain_query_filter: ChainQueryFilter::new(),
                activity_request: ActivityRequest::Full,
            },
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

// Checks that when there are multiple authorities returning signature bundles and multiple sessions
// happening between a pair of agents, the extra signature bundles beyond the first one received
// are correctly handled. I.e. receiving further signature bundles after a new session has started
// will not impact the new session.
// This is trying to check a race condition, so it's probably expected that the test won't always
// correctly test the problem. However, it is a scenario we've seen fail in Wind Tunnel, so it's
// worth trying to test here. Seeing flakes from this test is likely a signal that something has
// regressed.
#[tokio::test(flavor = "multi_thread")]
async fn signature_bundle_noise() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(5, config).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CounterSigning]).await;
    let apps = conductors.setup_app("app", &[dna]).await.unwrap();

    let cells = apps.cells_flattened();

    for i in 0..5 {
        // Need an initialised source chain for countersigning, so commit anything
        let zome = cells[i].zome(TestWasm::CounterSigning);
        let _: ActionHash = conductors[i]
            .call_fallible(&zome, "create_a_thing", ())
            .await
            .unwrap();

        // Also want to make sure everyone can see everyone else
        conductors[i]
            .require_initial_gossip_activity_for_cell(&cells[i], 4, Duration::from_secs(10))
            .await
            .unwrap();
    }

    let alice_zome = cells[0].zome(TestWasm::CounterSigning);
    let bob_zome = cells[1].zome(TestWasm::CounterSigning);

    // Run multiple sessions, one after another
    for _ in 0..5 {
        let preflight_request: PreflightRequest = conductors[0]
            .call_fallible(
                &alice_zome,
                "generate_countersigning_preflight_request",
                vec![
                    (cells[0].agent_pubkey().clone(), vec![Role(0)]),
                    (cells[1].agent_pubkey().clone(), vec![]),
                ],
            )
            .await
            .unwrap();
        let alice_acceptance: PreflightRequestAcceptance = conductors[0]
            .call_fallible(
                &cells[0].zome(TestWasm::CounterSigning),
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await
            .unwrap();
        let alice_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
                response
            } else {
                unreachable!(
                    "Expected PreflightRequestAcceptance::Accepted, got {:?}",
                    alice_acceptance
                );
            };
        let bob_acceptance: PreflightRequestAcceptance = conductors[1]
            .call_fallible(
                &bob_zome,
                "accept_countersigning_preflight_request",
                preflight_request.clone(),
            )
            .await
            .unwrap();
        let bob_response =
            if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
                response
            } else {
                unreachable!();
            };

        commit_session_with_retry(
            &conductors[0],
            &cells[0],
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await;
        commit_session_with_retry(
            &conductors[1],
            &cells[1],
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await;

        let alice_rx = conductors[0].subscribe_to_app_signals("app".into());
        let bob_rx = conductors[1].subscribe_to_app_signals("app".into());

        wait_for_completion(alice_rx, preflight_request.app_entry_hash.clone()).await;
        wait_for_completion(bob_rx, preflight_request.app_entry_hash).await;
    }
}

async fn commit_session_with_retry(
    conductor: &SweetConductor,
    cell: &SweetCell,
    responses: Vec<PreflightResponse>,
) {
    for _ in 0..5 {
        match conductor
            .call_fallible::<_, (ActionHash, EntryHash)>(
                &cell.zome(TestWasm::CounterSigning),
                "create_a_countersigned_thing_with_entry_hash",
                responses.clone(),
            )
            .await
        {
            Ok(_) => {
                break;
            }
            Err(ConductorApiError::CellError(CellError::WorkflowError(e))) => {
                if let WorkflowError::SourceChainError(SourceChainError::IncompleteCommit(_)) = *e {
                    // retryable error, missing DHT dependencies
                    continue;
                } else {
                    panic!("Expected IncompleteCommit error, got: {:?}", e);
                }
            }
            Err(e) => {
                panic!(
                    "Unexpected error while committing countersigning entry: {:?}",
                    e
                );
            }
        }
    }
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
    wait_for_abandoned(
        conductors[1].subscribe_to_app_signals("app".into()),
        preflight_request.app_entry_hash.clone(),
    )
    .await;

    // Alice session should also get abandoned
    wait_for_abandoned(
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
    await_consistency(60, [alice, bob, &carol]).await.unwrap();
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
    await_consistency(60, [alice, bob, &carol]).await.unwrap();
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
