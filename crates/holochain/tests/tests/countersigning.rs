use hdk::prelude::{PreflightRequest, PreflightRequestAcceptance, Timestamp};
use holo_hash::{ActionHash, EntryHash};
use holochain::conductor::api::error::{ConductorApiError, ConductorApiResult};
use holochain::conductor::CellError;
use holochain::core::workflow::WorkflowError;
use holochain::sweettest::{
    await_consistency, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
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
use std::ops::Add;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Receiver;

#[tokio::test(flavor = "multi_thread")]
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

    // Allow bootstrapping so that peers can find each other, but disable publish and recent gossip.
    // The only way peers can get data is through get requests!
    let config = SweetConductorConfig::rendezvous(true).historical_only();
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

#[tokio::test(flavor = "multi_thread")]
async fn ruin_somebody_elses_day_without_realising() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true);
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
    let carol_zome = carol.zome(TestWasm::CounterSigning);
    let _: ActionHash = conductors[2]
        .call_fallible(&carol_zome, "create_a_thing", ())
        .await
        .unwrap();

    await_consistency(30, vec![alice, bob, carol])
        .await
        .unwrap();

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

    // Alice doesn't realise that Bob is a very bad man, and goes ahead and commits
    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![alice_response.clone(), bob_response.clone()],
        )
        .await
        .unwrap();

    // Bob proceeds to laugh maniacally and not commit

    // Let's wait for the session to time out and see how badly this ends for Alice
    let end = Instant::now().add(
        (preflight_request.session_times.end - Timestamp::now())
            .unwrap()
            .to_std()
            .unwrap(),
    );
    tokio::time::sleep_until(end.into()).await;

    // Okay, so the chain lock has timed out and Alice should be able to commit now
    let _: ActionHash = conductors[0]
        .call_fallible(&alice_zome, "create_a_thing", ())
        .await
        .unwrap();

    // Alice continues to exist on the network and see other people's data. Sadly her future actions
    // effectively go into limbo because they can't be validated.

    // Let's wait for the session to time out so that the chain lock is released. Then Alice can
    // get on with her important task of ruining Carol's day.
    let end = Instant::now().add(
        (preflight_request.session_times.end - Timestamp::now())
            .unwrap()
            .to_std()
            .unwrap_or(Duration::from_secs(2)),
    );
    tokio::time::sleep_until(end.into()).await;

    // Poor innocent Carol, not realising what Bob has done to Alice, tries to transact with her.

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = conductors[2]
        .call_fallible(
            &carol_zome,
            "generate_countersigning_preflight_request_fast",
            vec![
                (carol.agent_pubkey().clone(), vec![Role(0)]),
                (alice.agent_pubkey().clone(), vec![]),
            ],
        )
        .await
        .unwrap();
    let carol_acceptance: PreflightRequestAcceptance = conductors[2]
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
            unreachable!("Got carol response {:?}", carol_acceptance);
        };
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

    // Carol, with a strange feeling that something is wrong that she can't explain, tries to commit anyway
    let (_, _): (ActionHash, EntryHash) = conductors[2]
        .call_fallible(
            &carol_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![carol_response.clone(), alice_response.clone()],
        )
        .await
        .unwrap();

    // Alice, still unaware of Bob's treachery, tries to commit again
    let (_, _): (ActionHash, EntryHash) = conductors[0]
        .call_fallible(
            &alice_zome,
            "create_a_countersigned_thing_with_entry_hash",
            vec![carol_response.clone(), alice_response.clone()],
        )
        .await
        .unwrap();

    // The session should complete because Alice's chain head is fine, that's published. There's just
    // some junk on her chain that means her state isn't really valid.
    let carol_rx = conductors[2].subscribe_to_app_signals("app".into());
    let alice_rx = conductors[0].subscribe_to_app_signals("app".into());

    wait_for_completion(carol_rx, preflight_request.app_entry_hash.clone()).await;
    wait_for_completion(alice_rx, preflight_request.app_entry_hash).await;

    // So what now? Well say Alice was supposed to have been transferred some resource by Bob. Bob
    // never actually agreed to send that but Alice has a record of it. It's only on her authored
    // chain and not the DHT so, it's not something that the network is aware of. But Alice isn't
    // really aware either. She has no way to detect that her chain is in an invalid state. She
    // knows her countersigning session timed out, but she doesn't necessarily know that the transaction
    // still sitting on her chain is failed.
    // By continuing to participate in the network, Alice has actually become a bad actor now.
    // Even that though, will only be discovered by full validation of her state against other peers.
    // Until somebody notices that Alice has using a transaction that only has, that doesn't appear on
    // Bob's chain - she will go on in her ethereal state, hurting everyone she touches.

    // There's no need to stop here though. Carol is one step further away from the original mistake,
    // but she will now go on to hurt others.
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
