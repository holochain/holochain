//! Test countersigning session interaction over websockets with full Holochain conductor.
//!
//! Tests run the Holochain binary and communicate over websockets.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use hdk::prelude::{CapAccess, GrantZomeCallCapabilityPayload, GrantedFunctions, ZomeCallCapGrant};
use hdk::prelude::{
    CapSecret, CellId, FunctionName, PreflightRequest, PreflightRequestAcceptance, Role,
};
use holo_hash::{ActionHash, AgentPubKey};
use holochain::prelude::{CountersigningSessionState, DhtOp, Signal, SystemSignal};
use holochain::sweettest::{authenticate_app_ws_client, websocket_client_by_port, WsPollRecv};
use holochain_conductor_api::conductor::{ConductorTuningParams, KeystoreConfig};
use holochain_conductor_api::AppRequest;
use holochain_conductor_api::{AdminRequest, AdminResponse, AppResponse};
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError};
use holochain_types::test_utils::{fake_dna_zomes, write_fake_dna_file};
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::{ReceiveMessage, WebsocketReceiver, WebsocketSender};
use kitsune_p2p_types::config::TransportConfig;

use arbitrary::Arbitrary;
use ed25519_dalek::SigningKey;
use matches::assert_matches;
use rand::rngs::OsRng;
use serde::{de::DeserializeOwned, Serialize};
use tempfile::TempDir;
use url2::Url2;

use crate::tests::test_utils::{
    attach_app_interface, call_zome_fn, call_zome_fn_fallible, check_timeout, create_config,
    register_and_install_dna, start_holochain_with_lair, start_local_services, write_config,
    SupervisedChild,
};

const APP_ID: &str = "test";

// Test countersigning interaction calls.
// - two agents on two conductors
// - alice commits a countersigned entry, bob does not commit, and alice shuts down the conductor (puts her session in unresolvable state)
// - alice makes call to abandon the session
// - again alice and bob accept a new preflight request
// - bob commits the countersigned entry and shuts down (session unresolvable)
// - alice commits the countersigned entry too while bob is offline and restarts her conductor (session unresolvable)
// - alice makes call to publish entry
// - bob starts up conductor again
// - await dht sync
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn countersigning_session_interaction_calls() {
    use holochain::{
        conductor::{api::error::ConductorApiError, error::ConductorError},
        prelude::CountersigningError,
    };

    holochain_trace::test_run();

    // Start local bootstrap and signal servers.
    let (_local_services, bootstrap_url_recv, signal_url_recv) = start_local_services().await;
    let bootstrap_url = bootstrap_url_recv.await.unwrap();
    let signal_url = signal_url_recv.await.unwrap();

    let network_seed = uuid::Uuid::new_v4().to_string();

    // Set up two agents on two conductors.
    let mut alice = setup_agent(
        bootstrap_url.clone(),
        signal_url.clone(),
        network_seed.clone(),
    )
    .await;

    // Attach app interface to Alice's conductor.
    let (alice_app_tx, mut alice_app_rx) = connect_app_interface(&mut alice).await;
    // Spawn task listening to app socket messages, preventing app socket to be dropped.
    tokio::spawn(async move { while alice_app_rx.recv::<AppResponse>().await.is_ok() {} });

    let mut bob = setup_agent(bootstrap_url, signal_url, network_seed.clone()).await;

    // Attach app interface to Bob's conductor.
    let (bob_app_tx, mut bob_app_rx) = connect_app_interface(&mut bob).await;
    // Spawn task listening to app socket messages, preventing app socket to be dropped.
    // Bob will later listen for an abandoned session signal.
    let (bob_session_abandonded_tx, mut bob_session_abandonded_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        while let Ok(ReceiveMessage::Signal(signal)) = bob_app_rx.recv::<AppResponse>().await {
            match Signal::try_from_vec(signal).unwrap() {
                Signal::System(SystemSignal::AbandonedCountersigning(entry_hash)) => {
                    let _ = bob_session_abandonded_tx.send(entry_hash).await;
                }
                _ => unreachable!(),
            }
        }
    });

    // Await peers to discover each other.
    tokio::time::timeout(
        Duration::from_secs(5),
        expect_bootstrapping_completed(&[&alice, &bob]),
    )
    .await
    .unwrap();

    tracing::info!("Agents set up and aware of each other.\n");

    // Initialize Alice's source chain.
    let _: ActionHash = call_zome(&alice, &alice_app_tx, "create_a_thing", &()).await;

    // Initialize Bob's source chain.
    let _: ActionHash = call_zome(&bob, &bob_app_tx, "create_a_thing", &()).await;

    // Await DHT sync of both agents.
    tokio::time::timeout(Duration::from_secs(30), await_dht_sync(&[&alice, &bob]))
        .await
        .unwrap();

    // Countersigning session state should not be in Alice's conductor memory yet.
    assert_matches!(get_session_state(&alice.cell_id, &alice_app_tx).await, None);
    // Countersigning session state should not be in Bob's conductor memory yet.
    assert_matches!(get_session_state(&bob.cell_id, &bob_app_tx).await, None);

    // Abandoning a session of a non-existing cell should return an error.
    let response: AppResponse = request(
        AppRequest::AbandonCountersigningSession(Box::new(bob.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    let expected_error = AppResponse::Error(
        ConductorApiError::ConductorError(ConductorError::CountersigningError(
            CountersigningError::WorkspaceDoesNotExist(bob.cell_id.clone()),
        ))
        .into(),
    );
    assert_eq!(format!("{:?}", response), format!("{:?}", expected_error));

    // Abandoning a non-existing session of an existing cell should return an error.
    let response: AppResponse = request(
        AppRequest::AbandonCountersigningSession(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    let expected_error = AppResponse::Error(
        ConductorApiError::ConductorError(ConductorError::CountersigningError(
            CountersigningError::SessionNotFound(alice.cell_id.clone()),
        ))
        .into(),
    );
    assert_eq!(format!("{:?}", response), format!("{:?}", expected_error));

    // Publishing a session of a non-existing cell should return an error.
    let response: AppResponse = request(
        AppRequest::PublishCountersigningSession(Box::new(bob.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    let expected_error = AppResponse::Error(
        ConductorApiError::ConductorError(ConductorError::CountersigningError(
            CountersigningError::WorkspaceDoesNotExist(bob.cell_id.clone()),
        ))
        .into(),
    );
    assert_eq!(format!("{:?}", response), format!("{:?}", expected_error));

    // Publishing a non-existing session of an existing cell should return an error.
    let response: AppResponse = request(
        AppRequest::PublishCountersigningSession(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    let expected_error = AppResponse::Error(
        ConductorApiError::ConductorError(ConductorError::CountersigningError(
            CountersigningError::SessionNotFound(alice.cell_id.clone()),
        ))
        .into(),
    );
    assert_eq!(format!("{:?}", response), format!("{:?}", expected_error));

    // Set up the session and accept it for both agents.
    let preflight_request: PreflightRequest = call_zome(
        &alice,
        &alice_app_tx,
        "generate_countersigning_preflight_request_fast", // 10 sec timeout
        &[
            (alice.cell_id.agent_pubkey().clone(), vec![Role(0)]),
            (bob.cell_id.agent_pubkey().clone(), vec![]),
        ],
    )
    .await;
    let alice_acceptance: PreflightRequestAcceptance = call_zome(
        &alice,
        &alice_app_tx,
        "accept_countersigning_preflight_request",
        &preflight_request,
    )
    .await;
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = call_zome(
        &bob,
        &bob_app_tx,
        "accept_countersigning_preflight_request",
        &preflight_request,
    )
    .await;
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    // Countersigning session state should exist for both agents and be in "Accepted" state.
    assert_matches!(
        get_session_state(&alice.cell_id, &alice_app_tx).await,
        Some(CountersigningSessionState::Accepted(_))
    );
    assert_matches!(
        get_session_state(&bob.cell_id, &bob_app_tx).await,
        Some(CountersigningSessionState::Accepted(_))
    );

    // Abandoning a session in a resolvable state should not be possible and return an error.
    let response: AppResponse = request(
        AppRequest::AbandonCountersigningSession(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    let expected_error = AppResponse::Error(
        ConductorApiError::ConductorError(ConductorError::CountersigningError(
            CountersigningError::SessionNotUnresolvable(alice.cell_id.clone()),
        ))
        .into(),
    );
    assert_eq!(format!("{:?}", response), format!("{:?}", expected_error));

    // Publishing a session in a resolvable state should not be possible and return an error.
    let response: AppResponse = request(
        AppRequest::PublishCountersigningSession(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    let expected_error = AppResponse::Error(
        ConductorApiError::ConductorError(ConductorError::CountersigningError(
            CountersigningError::SessionNotUnresolvable(alice.cell_id.clone()),
        ))
        .into(),
    );
    assert_eq!(format!("{:?}", response), format!("{:?}", expected_error));

    // Alice commits the countersigning entry. Up to 5 retries in case Bob's chain head can not be fetched
    // immediately.
    for _ in 0..5 {
        let response = call_zome_fn_fallible(
            &alice_app_tx,
            alice.cell_id.clone(),
            &alice.signing_keypair,
            alice.cap_secret,
            TestWasm::CounterSigning.coordinator_zome_name(),
            "create_a_countersigned_thing_with_entry_hash".into(),
            &[alice_response.clone(), bob_response.clone()],
        )
        .await;

        if let AppResponse::ZomeCalled(_) = response {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    tracing::info!("Alice committed countersigned entry - restarting conductor to provoke unresolvable state...\n");

    // Restart Alice's conductor to put countersigning session in Unresolvable state.
    let mut alice = restart_conductor(alice).await;

    // Attach app interface to Alice's conductor.
    let (alice_app_tx, mut alice_app_rx) = connect_app_interface(&mut alice).await;
    // Spawn task listening to app socket messages, preventing app socket to be dropped.
    tokio::spawn(async move { while alice_app_rx.recv::<AppResponse>().await.is_ok() {} });

    // Alice's session should be in state Unresolvable with 1 attempted resolution.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state = get_session_state(&alice.cell_id, &alice_app_tx).await;
            if let Some(CountersigningSessionState::Unknown {
                resolution: summary,
                ..
            }) = state
            {
                if summary.attempts == 1 {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    // Bob's session should still be in Accepted state.
    assert_matches!(
        get_session_state(&bob.cell_id, &bob_app_tx).await,
        Some(CountersigningSessionState::Accepted(_))
    );

    // Alice abandons the session.
    let response: AppResponse = request(
        AppRequest::AbandonCountersigningSession(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    assert_matches!(response, AppResponse::CountersigningSessionAbandoned);

    // Session should be abandoned and can not be abandoned again.
    let response: AppResponse = request(
        AppRequest::AbandonCountersigningSession(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    let expected_error = AppResponse::Error(
        ConductorApiError::ConductorError(ConductorError::CountersigningError(
            CountersigningError::SessionNotFound(alice.cell_id.clone()),
        ))
        .into(),
    );
    assert_eq!(format!("{:?}", response), format!("{:?}", expected_error));

    // Alice's session should be gone.
    assert_matches!(get_session_state(&alice.cell_id, &alice_app_tx).await, None);
    // Bob's session should still be in Accepted state.
    assert_matches!(
        get_session_state(&bob.cell_id, &bob_app_tx).await,
        Some(CountersigningSessionState::Accepted(_))
    );

    tracing::info!("Alice abandoned session.\n");

    // Await Bob's session to be abandoned due to timeout.
    let abandoned_session_entry_hash = bob_session_abandonded_rx.recv().await.unwrap();
    assert_eq!(
        abandoned_session_entry_hash,
        bob_response.request.app_entry_hash
    );
    // Bob's session should be gone too.
    assert_matches!(get_session_state(&bob.cell_id, &bob_app_tx).await, None);

    // Await DHT sync.
    tokio::time::timeout(Duration::from_secs(30), await_dht_sync(&[&alice, &bob]))
        .await
        .unwrap();

    tracing::info!("Starting over with a new countersigning session.\n");

    // Start over. Alice commits the countersigned entry again.
    // Set up the session and accept it for both agents.
    let preflight_request: PreflightRequest = call_zome(
        &alice,
        &alice_app_tx,
        "generate_countersigning_preflight_request_fast", // 10 sec timeout
        &[
            (alice.cell_id.agent_pubkey().clone(), vec![Role(0)]),
            (bob.cell_id.agent_pubkey().clone(), vec![]),
        ],
    )
    .await;
    let alice_acceptance: PreflightRequestAcceptance = call_zome(
        &alice,
        &alice_app_tx,
        "accept_countersigning_preflight_request",
        &preflight_request,
    )
    .await;
    let alice_response =
        if let PreflightRequestAcceptance::Accepted(ref response) = alice_acceptance {
            response
        } else {
            unreachable!();
        };
    let bob_acceptance: PreflightRequestAcceptance = call_zome(
        &bob,
        &bob_app_tx,
        "accept_countersigning_preflight_request",
        &preflight_request,
    )
    .await;
    let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
        response
    } else {
        unreachable!();
    };

    // Countersigning session state should exist for both agents and be in "Accepted" state.
    assert_matches!(
        get_session_state(&alice.cell_id, &alice_app_tx).await,
        Some(CountersigningSessionState::Accepted(_))
    );
    assert_matches!(
        get_session_state(&bob.cell_id, &bob_app_tx).await,
        Some(CountersigningSessionState::Accepted(_))
    );

    // Bob commits entry and shuts down.
    for _ in 0..5 {
        let response = call_zome_fn_fallible(
            &bob_app_tx,
            bob.cell_id.clone(),
            &bob.signing_keypair,
            bob.cap_secret,
            TestWasm::CounterSigning.coordinator_zome_name(),
            "create_a_countersigned_thing_with_entry_hash".into(),
            &[alice_response.clone(), bob_response.clone()],
        )
        .await;

        if let AppResponse::ZomeCalled(_) = response {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    tracing::info!("Bob committed countersigned entry - shutting down conductor.\n");

    let bob_config = shutdown_agent(bob);

    // Alice commits countersigned entry.
    for _ in 0..5 {
        let response = call_zome_fn_fallible(
            &alice_app_tx,
            alice.cell_id.clone(),
            &alice.signing_keypair,
            alice.cap_secret,
            TestWasm::CounterSigning.coordinator_zome_name(),
            "create_a_countersigned_thing_with_entry_hash".into(),
            &[alice_response.clone(), bob_response.clone()],
        )
        .await;

        if let AppResponse::ZomeCalled(_) = response {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    tracing::info!("Alice committed countersigned entry - restarting conductor to provoke unresolvable state...\n");

    // Restart Alice's conductor to put countersigning session in Unresolvable state.
    let mut alice = restart_conductor(alice).await;

    // Attach app interface to Alice's conductor.
    let (alice_app_tx, mut alice_app_rx) = connect_app_interface(&mut alice).await;
    // Spawn task listening for signals.
    tokio::spawn(async move { while alice_app_rx.recv::<AppResponse>().await.is_ok() {} });

    // Alice's session should be in state Unresolvable.
    // Leave some time for the countersigning workflow to refresh the session in memory.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if matches!(
                get_session_state(&alice.cell_id, &alice_app_tx).await,
                Some(CountersigningSessionState::Unknown { .. })
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    tracing::info!("Alice about to force-publish countersigned entry.\n");

    // Alice publishes the unresolved session.
    let response: AppResponse = request(
        AppRequest::PublishCountersigningSession(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    assert_matches!(response, AppResponse::CountersigningSessionPublished);

    assert_matches!(get_session_state(&alice.cell_id, &alice_app_tx).await, None);

    tracing::info!("Alice published entry. Starting up Bob again.\n");

    // Bring Bob back up.
    let mut bob = startup_agent(bob_config).await;

    let (bob_app_tx, mut bob_app_rx) = connect_app_interface(&mut bob).await;
    // Spawn task listening for successful countersigning session signal.
    let (bob_successful_session_tx, mut bob_successful_session_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        while let Ok(ReceiveMessage::Signal(signal)) = bob_app_rx.recv::<AppResponse>().await {
            match Signal::try_from_vec(signal).unwrap() {
                Signal::System(SystemSignal::SuccessfulCountersigning(entry)) => {
                    tracing::info!("Bob received countersigning session success signal.\n");
                    let _ = bob_successful_session_tx.clone().send(entry).await;
                }
                _ => unreachable!(),
            }
        }
    });

    tokio::time::timeout(Duration::from_secs(10), bob_successful_session_rx.recv())
        .await
        .unwrap();

    assert_matches!(get_session_state(&bob.cell_id, &bob_app_tx).await, None);

    tracing::info!("Sessions resolved successfully. Awaiting DHT sync...");

    // Syncing takes long because Alice's publish loop pauses for a minute.
    tokio::time::timeout(Duration::from_secs(120), await_dht_sync(&[&alice, &bob]))
        .await
        .unwrap();
}

struct Agent {
    admin_tx: WebsocketSender,
    admin_port: u16,
    cell_id: CellId,
    signing_keypair: SigningKey,
    cap_secret: CapSecret,
    config_path: PathBuf,
    _admin_rx: WsPollRecv,
    _holochain: SupervisedChild,
}

async fn setup_agent(bootstrap_url: String, signal_url: String, network_seed: String) -> Agent {
    let admin_port = 0;
    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.into_path();
    let environment_path = path.clone();
    let mut config = create_config(admin_port, environment_path.into());
    config.keystore = KeystoreConfig::LairServerInProc { lair_root: None };
    config.tuning_params = Some(ConductorTuningParams {
        countersigning_resolution_retry_delay: Some(Duration::from_secs(5)),
        min_publish_interval: Some(Duration::from_secs(5)),
        ..Default::default()
    });
    config.network.bootstrap_service = Some(Url2::parse(bootstrap_url));
    config.network.transport_pool = vec![TransportConfig::WebRTC {
        signal_url,
        webrtc_config: None,
    }];
    let config_path = write_config(path.clone(), &config);

    let (_holochain, admin_port) = start_holochain_with_lair(config_path.clone(), true).await;
    let admin_port = admin_port.await.unwrap();

    let (mut admin_tx, admin_rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _admin_rx = WsPollRecv::new::<AdminResponse>(admin_rx);

    let dna = fake_dna_zomes(
        &network_seed,
        vec![(
            TestWasm::CounterSigning.into(),
            TestWasm::CounterSigning.into(),
        )],
    );

    // Install Dna.
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
    let cell_id = register_and_install_dna(&mut admin_tx, fake_dna_path, None, "".into(), 10000)
        .await
        .unwrap();

    // Activate cells.
    let request = AdminRequest::EnableApp {
        installed_app_id: APP_ID.to_string(),
    };
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await.unwrap();
    assert_matches!(response, AdminResponse::AppEnabled { .. });

    // Generate signing key pair.
    let mut rng = OsRng;
    let signing_keypair = ed25519_dalek::SigningKey::generate(&mut rng);
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.verifying_key().as_bytes().to_vec());

    // Grant zome call capability for agent.
    let functions = GrantedFunctions::All;

    let mut buf = arbitrary::Unstructured::new(&[]);
    let cap_secret = CapSecret::arbitrary(&mut buf).unwrap();

    let mut assignees = BTreeSet::new();
    assignees.insert(signing_key.clone());

    let request = AdminRequest::GrantZomeCallCapability(Box::new(GrantZomeCallCapabilityPayload {
        cell_id: cell_id.clone(),
        cap_grant: ZomeCallCapGrant {
            tag: "".into(),
            access: CapAccess::Assigned {
                secret: cap_secret,
                assignees,
            },
            functions,
        },
    }));
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await.unwrap();
    assert_matches!(response, AdminResponse::ZomeCallCapabilityGranted);

    Agent {
        admin_tx,
        admin_port,
        cell_id,
        signing_keypair,
        cap_secret,
        config_path,
        _admin_rx,
        _holochain,
    }
}

fn shutdown_agent(agent: Agent) -> (PathBuf, CellId, SigningKey, CapSecret) {
    let Agent {
        config_path,
        cell_id,
        signing_keypair,
        cap_secret,
        admin_tx,
        _admin_rx,
        _holochain,
        ..
    } = agent;
    drop(_holochain);
    drop(admin_tx);
    drop(_admin_rx);
    (config_path, cell_id, signing_keypair, cap_secret)
}

async fn startup_agent(config: (PathBuf, CellId, SigningKey, CapSecret)) -> Agent {
    let (config_path, cell_id, signing_keypair, cap_secret) = config;

    let (_holochain, admin_port) = start_holochain_with_lair(config_path.clone(), true).await;
    let admin_port = admin_port.await.unwrap();
    let (admin_tx, _admin_rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _admin_rx = WsPollRecv::new::<AdminResponse>(_admin_rx);

    Agent {
        admin_tx,
        admin_port,
        cell_id,
        signing_keypair,
        cap_secret,
        config_path,
        _admin_rx,
        _holochain,
    }
}

async fn restart_conductor(agent: Agent) -> Agent {
    let agent_config = shutdown_agent(agent);
    startup_agent(agent_config).await
}

async fn connect_app_interface(agent: &mut Agent) -> (WebsocketSender, WebsocketReceiver) {
    let app_port = attach_app_interface(&agent.admin_tx, None).await;
    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    authenticate_app_ws_client(app_tx.clone(), agent.admin_port, APP_ID.to_string()).await;
    (app_tx, app_rx)
}

async fn expect_bootstrapping_completed(agents: &[&Agent]) {
    loop {
        let agent_requests = agents.iter().map(|agent| async {
            match request(AdminRequest::AgentInfo { cell_id: None }, &agent.admin_tx).await {
                AdminResponse::AgentInfo(agent_infos) => agent_infos.len() == agents.len(),
                _ => unreachable!(),
            }
        });
        let all_agents_visible = futures::future::join_all(agent_requests)
            .await
            .into_iter()
            .all(|result| result);
        if all_agents_visible {
            break;
        } else {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

async fn await_dht_sync(agents: &[&Agent]) {
    loop {
        let requests = agents.iter().map(|agent| async {
            match request(
                AdminRequest::DumpFullState {
                    cell_id: Box::new(agent.cell_id.clone()),
                    dht_ops_cursor: None,
                },
                &agent.admin_tx,
            )
            .await
            {
                AdminResponse::FullStateDumped(state) => {
                    let mut dht = state.integration_dump.integrated;
                    sort_dht(&mut dht);
                    dht
                }
                _ => unreachable!(),
            }
        });

        let dhts = futures::future::join_all(requests).await;
        let dhts_synced = dhts[1..].iter().all(|dht| *dht == dhts[0]);
        if dhts_synced {
            break;
        } else {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

fn sort_dht(dht: &mut [DhtOp]) {
    dht.sort_by(|a, b| match a {
        DhtOp::ChainOp(chain_op_a) => {
            if let DhtOp::ChainOp(chain_op_b) = b {
                let type_a = format!(
                    "{}{}{}",
                    chain_op_a.get_type(),
                    chain_op_a.author(),
                    chain_op_a.action().action_seq(),
                );
                let type_b = format!(
                    "{}{}{}",
                    chain_op_b.get_type(),
                    chain_op_b.author(),
                    chain_op_b.action().action_seq(),
                );
                type_a.partial_cmp(&type_b).unwrap()
            } else {
                Ordering::Greater
            }
        }
        _ => unimplemented!(),
    });
}

async fn request<Request, Response>(request: Request, tx: &WebsocketSender) -> Response
where
    Request: std::fmt::Debug,
    SerializedBytes: TryFrom<Request, Error = SerializedBytesError>,
    Response: std::fmt::Debug + DeserializeOwned,
{
    let response = tx.request(request);
    check_timeout::<Response>(response, 6000).await.unwrap()
}

async fn get_session_state(
    cell_id: &CellId,
    app_tx: &WebsocketSender,
) -> Option<CountersigningSessionState> {
    match request(
        AppRequest::GetCountersigningSessionState(Box::new(cell_id.clone())),
        app_tx,
    )
    .await
    {
        AppResponse::CountersigningSessionState(maybe_state) => *maybe_state,
        _ => unreachable!(),
    }
}

async fn call_zome<I, O>(
    agent: &Agent,
    app_tx: &WebsocketSender,
    fn_name: impl Into<FunctionName>,
    input: &I,
) -> O
where
    I: Serialize + std::fmt::Debug,
    O: DeserializeOwned + std::fmt::Debug,
{
    let zome_name = TestWasm::CounterSigning.coordinator_zome_name();
    call_zome_fn(
        app_tx,
        agent.cell_id.clone(),
        &agent.signing_keypair,
        agent.cap_secret,
        zome_name,
        fn_name.into(),
        input,
    )
    .await
    .decode()
    .unwrap()
}
