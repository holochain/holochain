//! Test countersigning session interaction with full Holochain conductor over websockets.
//!
//! Tests run the Holochain binary and communicate over websockets.

use std::collections::BTreeSet;
use std::time::Duration;

use arbitrary::Arbitrary;
use ed25519_dalek::ed25519::signature::SignerMut;
use ed25519_dalek::SigningKey;
use hdk::prelude::{
    CapAccess, ExternIO, GrantZomeCallCapabilityPayload, GrantedFunctions, Signature, Timestamp,
    ZomeCallCapGrant, ZomeCallUnsigned,
};
use hdk::prelude::{CapSecret, CellId, ChainQueryFilter, FunctionName, ZomeName};
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain::prelude::{Signal, SystemSignal};
use holochain::sweettest::{
    authenticate_app_ws_client, network, websocket_client_by_port, WsPollRecv,
};
use holochain_conductor_api::{AdminRequest, AdminResponse, AppResponse};
use holochain_conductor_api::{AppRequest, ZomeCall};
use holochain_types::test_utils::{fake_dna_zomes, write_fake_dna_file};
use holochain_wasm_test_utils::TestWasm;
use holochain_wasm_test_utils::TestWasmPair;
use holochain_websocket::{ReceiveMessage, WebsocketReceiver, WebsocketSender};
use kitsune_p2p_types::config::TransportConfig;
use matches::assert_matches;
use rand::rngs::OsRng;
use serde::{de::DeserializeOwned, Serialize};
use tempfile::TempDir;
use tokio::task::JoinHandle;
use url2::Url2;

use crate::tests::test_utils::SupervisedChild;
use crate::tests::test_utils::{
    attach_app_interface, call_zome_fn, check_timeout, create_config, grant_zome_call_capability,
    register_and_install_dna, start_holochain, write_config,
};

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn get_session_state() {
    use std::sync::Arc;

    use hdk::prelude::{
        ActivityRequest, AgentActivity, GetAgentActivityInput, PreflightRequest,
        PreflightRequestAcceptance, QueryFilter, Role,
    };
    use holo_hash::EntryHash;
    use holochain::prelude::CounterSigningSessionState;
    use opentelemetry_api::metrics::Counter;
    use tokio::sync::Mutex;
    use url2::Url2;

    use crate::tests::test_utils::{call_zome_fn_fallible, start_local_services};

    holochain_trace::test_run();

    // Start a local bootstrap server.
    let (_local_services, bootstrap_url_recv, signal_url_recv) = start_local_services().await;
    let bootstrap_url = bootstrap_url_recv.await.unwrap();
    let signal_url = signal_url_recv.await.unwrap();
    println!("bootstrap url {bootstrap_url}");
    println!("signal url {signal_url}");

    let network_seed = uuid::Uuid::new_v4().to_string();

    // Setup two agents on two conductors.
    let mut alice = setup_agent(
        bootstrap_url.clone(),
        signal_url.clone(),
        network_seed.clone(),
    )
    .await;

    // Alice Attach App Interface
    let app_port = attach_app_interface(&mut alice.admin_tx, None).await;

    let (alice_app_tx, mut alice_app_rx) = websocket_client_by_port(app_port).await.unwrap();
    authenticate_app_ws_client(alice_app_tx.clone(), alice.admin_port, "test".to_string()).await;

    tokio::spawn(async move {
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

    // Bob Attach App Interface
    let app_port = attach_app_interface(&mut bob.admin_tx, None).await;

    let (bob_app_tx, mut bob_app_rx) = websocket_client_by_port(app_port).await.unwrap();
    authenticate_app_ws_client(bob_app_tx.clone(), bob.admin_port, "test".to_string()).await;

    let session_abandoned = tokio::spawn(async move {
        if let Ok(ReceiveMessage::Signal(signal)) = bob_app_rx.recv::<AppResponse>().await {
            match ExternIO::from(signal).decode::<Signal>().unwrap() {
                Signal::System(system_signal) => match system_signal {
                    SystemSignal::AbandonedCountersigning(entry) => {
                        println!("bob received system signal to abandon countersigning with entry {entry:?}");
                        true
                    }
                    _ => false,
                },
                _ => false,
            }
        } else {
            false
        }
    });

    // Need an initialised source chain for countersigning, so commit anything.
    // Alice
    let result: ActionHash = call_zome(&alice, &alice_app_tx, "create_a_thing", &()).await;
    println!("result is {result:?}");

    // Countersigning session state should not be in Alice's conductor memory yet.
    let request = AppRequest::GetCounterSigningSessionState(Box::new(alice.cell_id.clone()));
    let response = alice_app_tx.request(request);
    let call_response = check_timeout(response, 6000).await.unwrap();
    match call_response {
        AppResponse::CounterSigningSessionState(maybe_state) => {
            assert_matches!(*maybe_state, None);
        }
        _ => panic!("unexpected countersigning session state response"),
    }

    // Bob
    let result: ActionHash = call_zome(&bob, &bob_app_tx, "create_a_thing", &()).await;
    println!("result is {result:?}");

    // Countersigning session state should not be in Bob's conductor memory yet.
    let request = AppRequest::GetCounterSigningSessionState(Box::new(bob.cell_id.clone()));
    let response = bob_app_tx.request(request);
    let call_response = check_timeout(response, 6000).await.unwrap();
    match call_response {
        AppResponse::CounterSigningSessionState(maybe_state) => {
            assert_matches!(*maybe_state, None);
        }
        _ => panic!("unexpected countersigning session state response"),
    }

    // Await consistency.
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Set up the session and accept it for both agents.
    let preflight_request: PreflightRequest = call_zome(
        &alice,
        &alice_app_tx,
        "generate_countersigning_preflight_request_fast",
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

    let request = AppRequest::GetCounterSigningSessionState(Box::new(alice.cell_id.clone()));
    let response = alice_app_tx.request(request);
    let call_response = check_timeout(response, 6000).await.unwrap();
    if let AppResponse::CounterSigningSessionState(session_state) = call_response {
        assert_matches!(
            *session_state,
            Some(CounterSigningSessionState::Accepted(_))
        );
    } else {
        unreachable!();
    }

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

    let request = AppRequest::GetCounterSigningSessionState(Box::new(bob.cell_id.clone()));
    let response = bob_app_tx.request(request);
    let call_response = check_timeout(response, 6000).await.unwrap();
    println!("call response {call_response:?}\n");
    if let AppResponse::CounterSigningSessionState(session_state) = call_response {
        assert_matches!(
            *session_state,
            Some(CounterSigningSessionState::Accepted(_))
        );
    } else {
        unreachable!();
    }

    // Alice commits the countersigning entry.
    let mut tries = 0;
    loop {
        tries += 1;
        println!("countersign commit entry try {tries}");
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
        println!("countersign commit entry response {response:?}\n");

        if let AppResponse::ZomeCalled(result) = response {
            println!(
                "result is {:?}",
                result.decode::<(ActionHash, EntryHash)>().unwrap()
            );
            break;
        } else if tries == 5 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let request = AppRequest::GetCounterSigningSessionState(Box::new(alice.cell_id.clone()));
    let response = alice_app_tx.request(request);
    let call_response: AppResponse = check_timeout(response, 6000).await.unwrap();
    println!("call response 2 {call_response:?}");
    if let AppResponse::CounterSigningSessionState(session_state) = call_response {
        println!("cs state {session_state:?}");
        assert_matches!(
            *session_state,
            Some(CounterSigningSessionState::Accepted(_))
        );
    } else {
        unreachable!();
    }

    let start = std::time::Instant::now();
    let val = session_abandoned.await;
    println!(
        "it took {:?} for the session to be abandoned {val:?}",
        start.elapsed()
    );
}

struct Agent {
    admin_tx: WebsocketSender,
    admin_port: u16,
    cell_id: CellId,
    signing_keypair: SigningKey,
    cap_secret: CapSecret,
    _holochain: SupervisedChild,
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
    println!("network config {:?}", config.network.is_tx5());
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
        _admin_rx,
        _holochain,
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
        agent.cap_secret.clone(),
        zome_name,
        fn_name.into(),
        input,
    )
    .await
    .decode()
    .unwrap()
}
