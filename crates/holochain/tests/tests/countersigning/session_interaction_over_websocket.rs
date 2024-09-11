//! Test countersigning session interaction over websockets with full Holochain conductor.
//!
//! Tests run the Holochain binary and communicate over websockets.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use arbitrary::Arbitrary;
use ed25519_dalek::SigningKey;
use hdk::prelude::{
    CapAccess, ExternIO, GrantZomeCallCapabilityPayload, GrantedFunctions, ZomeCallCapGrant,
};
use hdk::prelude::{CapSecret, CellId, FunctionName};
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain::prelude::{Signal, SystemSignal};
use holochain::sweettest::{authenticate_app_ws_client, websocket_client_by_port, WsPollRecv};
use holochain_conductor_api::AppRequest;
use holochain_conductor_api::{AdminRequest, AdminResponse, AppResponse};
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError};
use holochain_types::test_utils::{fake_dna_zomes, write_fake_dna_file};
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::{ReceiveMessage, WebsocketSender};
use kitsune_p2p_types::config::TransportConfig;
use matches::assert_matches;
use rand::rngs::OsRng;
use serde::{de::DeserializeOwned, Serialize};
use tempfile::TempDir;
use url2::Url2;

use crate::tests::test_utils::SupervisedChild;
use crate::tests::test_utils::{
    attach_app_interface, call_zome_fn, check_timeout, create_config, register_and_install_dna,
    start_holochain, write_config,
};

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn get_session_state() {
    use hdk::prelude::{PreflightRequest, PreflightRequestAcceptance, Role};
    use holochain::prelude::{CountersigningSessionState, SessionResolutionSummary};

    use crate::tests::test_utils::{call_zome_fn_fallible, start_local_services};

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
    let app_port = attach_app_interface(&mut alice.admin_tx, None).await;
    let (alice_app_tx, mut alice_app_rx) = websocket_client_by_port(app_port).await.unwrap();
    authenticate_app_ws_client(alice_app_tx.clone(), alice.admin_port, "test".to_string()).await;

    tokio::spawn(async move {
        // while let Ok(_) = alice_app_rx.recv::<AppResponse>().await {}
        while let Ok(ReceiveMessage::Signal(signal)) = alice_app_rx.recv::<AppResponse>().await {
            match ExternIO::from(signal).decode::<Signal>().unwrap() {
                Signal::System(system_signal) => match system_signal {
                    SystemSignal::AbandonedCountersigning(entry) => {
                        println!("alice received system signal to abandon countersigning with entry {entry:?}");
                    }
                    SystemSignal::SuccessfulCountersigning(entry) => {
                        println!("alice received system signal successful countersigning with entry {entry:?}");
                    }
                },
                _ => (),
            }
        }
    });

    let mut bob = setup_agent(bootstrap_url, signal_url, network_seed.clone()).await;

    // Attach app interface to Bob's conductor.
    let app_port = attach_app_interface(&mut bob.admin_tx, None).await;
    let (bob_app_tx, mut bob_app_rx) = websocket_client_by_port(app_port).await.unwrap();
    authenticate_app_ws_client(bob_app_tx.clone(), bob.admin_port, "test".to_string()).await;

    // Spawn task with app socket signal waiting for system signals.
    let (bob_session_abanded_tx, mut bob_session_abandonded_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        while let Ok(ReceiveMessage::Signal(signal)) = bob_app_rx.recv::<AppResponse>().await {
            match ExternIO::from(signal).decode::<Signal>().unwrap() {
                Signal::System(system_signal) => match system_signal {
                    SystemSignal::AbandonedCountersigning(entry) => {
                        let _ = bob_session_abanded_tx.clone().send(entry).await;
                    }
                    _ => unreachable!(),
                },
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

    // Initialize Alice's source chain.
    let _: ActionHash = call_zome(&alice, &alice_app_tx, "create_a_thing", &()).await;

    // Initialize Bob's source chain.
    let _: ActionHash = call_zome(&bob, &bob_app_tx, "create_a_thing", &()).await;

    // Await DHT sync of both agents.
    tokio::time::timeout(Duration::from_secs(10), await_dht_sync(&[&alice, &bob]))
        .await
        .unwrap();

    // Countersigning session state should not be in Alice's conductor memory yet.
    match request(
        AppRequest::GetCountersigningSessionState(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await
    {
        AppResponse::CountersigningSessionState(maybe_state) => {
            assert_matches!(*maybe_state, None);
        }
        _ => panic!("unexpected countersigning session state"),
    }
    // Countersigning session state should not be in Bob's conductor memory yet.
    match request(
        AppRequest::GetCountersigningSessionState(Box::new(bob.cell_id.clone())),
        &bob_app_tx,
    )
    .await
    {
        AppResponse::CountersigningSessionState(maybe_state) => {
            assert_matches!(*maybe_state, None);
        }
        _ => panic!("unexpected countersigning session state"),
    }

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
    match request(
        AppRequest::GetCountersigningSessionState(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await
    {
        AppResponse::CountersigningSessionState(maybe_state) => {
            assert_matches!(*maybe_state, Some(CountersigningSessionState::Accepted(_)))
        }
        _ => panic!("unexpected countersigning session state"),
    }
    match request(
        AppRequest::GetCountersigningSessionState(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await
    {
        AppResponse::CountersigningSessionState(maybe_state) => {
            assert_matches!(*maybe_state, Some(CountersigningSessionState::Accepted(_)))
        }
        _ => panic!("unexpected countersigning session state"),
    }

    // Alice commits the countersigning entry. Up to 5 retries in case Bob's chain head can not be fetched
    // immediately.
    for _ in 0..5 {
        let response = call_zome_fn_fallible(
            &alice_app_tx,
            alice.cell_id.clone(),
            &alice.signing_keypair,
            alice.cap_secret.clone(),
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

    // Restart Alice's conductor to put countersigning session in Unknown state.
    let mut alice = restart_conductor(alice).await;

    // Attach app interface to Bob's conductor.
    let alice_app_port = attach_app_interface(&mut alice.admin_tx, None).await;
    let (alice_app_tx, mut alice_app_rx) = websocket_client_by_port(alice_app_port).await.unwrap();
    authenticate_app_ws_client(alice_app_tx.clone(), alice.admin_port, "test".to_string()).await;

    // Spawn task with app socket signal waiting for system signals.
    // let (bob_session_abanded_tx, mut bob_session_abandonded_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        // while let Ok(_) = bob_app_rx.recv::<AppResponse>().await {}
        while let Ok(ReceiveMessage::Signal(signal)) = alice_app_rx.recv::<AppResponse>().await {
            match ExternIO::from(signal).decode::<Signal>().unwrap() {
                Signal::System(system_signal) => match system_signal {
                    SystemSignal::AbandonedCountersigning(entry) => {
                        println!("alice abandoned cs session signal {entry:?}");
                        // let _ = bob_session_abanded_tx.clone().send(entry).await;
                    }
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
    });

    // Alice's session should be in state Unknown with 1 attempted resolution.
    match request(
        AppRequest::GetCountersigningSessionState(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await
    {
        AppResponse::CountersigningSessionState(maybe_state) => {
            assert_matches!(
                *maybe_state,
                Some(CountersigningSessionState::Unknown { resolution: Some(SessionResolutionSummary { attempts, completion_attempts, .. }), .. }) if attempts == 1 && completion_attempts == 0
            );
        }
        _ => panic!("unexpected app response"),
    }
    // Bob's session should still be in Accepted state.
    match request(
        AppRequest::GetCountersigningSessionState(Box::new(bob.cell_id.clone())),
        &bob_app_tx,
    )
    .await
    {
        AppResponse::CountersigningSessionState(maybe_state) => {
            assert_matches!(*maybe_state, Some(CountersigningSessionState::Accepted(_)));
        }
        _ => panic!("unexpected app response"),
    }

    // Alice abandons the session.
    let a: AppResponse = request(
        AppRequest::AbandonCountersigningSession(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await;
    println!("a {a:?}");

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Alice's session should be in state Unknown with 1 attempted resolution.
    match request(
        AppRequest::GetCountersigningSessionState(Box::new(alice.cell_id.clone())),
        &alice_app_tx,
    )
    .await
    {
        AppResponse::CountersigningSessionState(maybe_state) => {
            println!("alice session state after abandoning is {maybe_state:?}");
            // assert_matches!(
            //     *maybe_state,
            //     Some(CountersigningSessionState::Unknown { resolution: Some(SessionResolutionSummary { attempts, completion_attempts, .. }), .. }) if attempts == 1 && completion_attempts == 0
            // );
        }
        _ => panic!("unexpected app response"),
    }
    // Bob's session should still be in Accepted state.
    match request(
        AppRequest::GetCountersigningSessionState(Box::new(bob.cell_id.clone())),
        &bob_app_tx,
    )
    .await
    {
        AppResponse::CountersigningSessionState(maybe_state) => {
            println!("bob session state after abandoning is {maybe_state:?}");
            // assert_matches!(*maybe_state, Some(CountersigningSessionState::Accepted(_)));
        }
        _ => panic!("unexpected app response"),
    }
}

struct Agent {
    admin_tx: WebsocketSender,
    admin_port: u16,
    cell_id: CellId,
    signing_keypair: SigningKey,
    cap_secret: CapSecret,
    _holochain: SupervisedChild,
    tmp_dir: TempDir,
    config_path: PathBuf,
    _admin_rx: WsPollRecv,
}

async fn setup_agent(bootstrap_url: String, signal_url: String, network_seed: String) -> Agent {
    let admin_port = 0;
    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let mut config = create_config(admin_port, environment_path.into());
    config.network.bootstrap_service = Some(Url2::parse(bootstrap_url));
    config.network.transport_pool = vec![TransportConfig::WebRTC {
        signal_url,
        webrtc_config: None,
    }];
    let config_path = write_config(path, &config);

    let (_holochain, admin_port) = start_holochain(config_path.clone()).await;
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

    // Install Dna
    let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
    let cell_id = register_and_install_dna(&mut admin_tx, fake_dna_path, None, "".into(), 10000)
        .await
        .unwrap();

    // Activate cells
    let request = AdminRequest::EnableApp {
        installed_app_id: "test".to_string(),
    };
    let response = admin_tx.request(request);
    let response = check_timeout(response, 3000).await.unwrap();
    assert_matches!(response, AdminResponse::AppEnabled { .. });

    // Generate signing key pair
    let mut rng = OsRng;
    let signing_keypair = ed25519_dalek::SigningKey::generate(&mut rng);
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.verifying_key().as_bytes().to_vec());

    // Grant zome call capability for agent
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
        tmp_dir,
        config_path,
        _admin_rx,
        _holochain,
    }
}

async fn restart_conductor(agent: Agent) -> Agent {
    let Agent {
        config_path,
        cell_id,
        signing_keypair,
        cap_secret,
        _holochain,
        tmp_dir,
        admin_tx,
        _admin_rx,
        ..
    } = agent;

    // Shut down conductor.
    drop(_holochain);
    drop(admin_tx);
    drop(_admin_rx);

    // Restart conductor.
    let (_holochain, admin_port) = start_holochain(config_path.clone()).await;
    let admin_port = admin_port.await.unwrap();
    let (admin_tx, _admin_rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _admin_rx = WsPollRecv::new::<AdminResponse>(_admin_rx);

    Agent {
        admin_tx,
        admin_port,
        cell_id,
        signing_keypair,
        cap_secret,
        tmp_dir,
        config_path,
        _admin_rx,
        _holochain,
    }
}

async fn expect_bootstrapping_completed(agents: &[&Agent]) {
    loop {
        let agent_requests = agents.into_iter().map(|agent| async {
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
        let requests = agents.into_iter().map(|agent| async {
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
                    let mut ops = state.integration_dump.integrated;
                    ops.sort();
                    ops
                }
                _ => unreachable!(),
            }
        });
        let dhts = futures::future::join_all(requests).await;
        let dhts_synced = dhts.iter().all(|dht| *dht == dhts[0]);
        if dhts_synced {
            break;
        } else {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
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
        agent.cap_secret.clone(),
        zome_name,
        fn_name.into(),
        input,
    )
    .await
    .decode()
    .unwrap()
}
