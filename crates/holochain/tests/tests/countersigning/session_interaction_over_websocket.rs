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
use holochain::sweettest::{authenticate_app_ws_client, websocket_client_by_port, WsPollRecv};
use holochain_conductor_api::{AdminRequest, AdminResponse, AppResponse};
use holochain_conductor_api::{AppRequest, ZomeCall};
use holochain_types::test_utils::{fake_dna_zomes, write_fake_dna_file};
use holochain_wasm_test_utils::TestWasm;
use holochain_wasm_test_utils::TestWasmPair;
use holochain_websocket::{WebsocketReceiver, WebsocketSender};
use matches::assert_matches;
use rand::rngs::OsRng;
use serde::{de::DeserializeOwned, Serialize};
use tempfile::TempDir;

use crate::tests::test_utils::SupervisedChild;
use crate::tests::test_utils::{
    attach_app_interface, call_zome_fn, check_timeout, create_config, grant_zome_call_capability,
    register_and_install_dna, start_holochain, write_config,
};

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn get_session_state() {
    use hdk::prelude::{
        ActivityRequest, AgentActivity, GetAgentActivityInput, PreflightRequest,
        PreflightRequestAcceptance, QueryFilter, Role,
    };

    holochain_trace::test_run();

    let alice = setup_agent(30000).await;
    let bob = setup_agent(30001).await;

    // Call Zome
    let result: ActionHash = call_zome(&alice, "create_a_thing", &()).await;
    println!("result is {result:?}");

    let request = AppRequest::GetCountersigningSessionState(Box::new(alice.cell_id.clone()));
    let response = alice.app_tx.request(request);
    let call_response = check_timeout(response, 6000).await.unwrap();
    match call_response {
        AppResponse::CountersigningSessionState(response) => {
            println!("res {:?}", response)
        }
        _ => panic!("unexpected zome call response"),
    }

    let result: ActionHash = call_zome(&bob, "create_a_thing", &()).await;
    println!("result is {result:?}");

    let request = AppRequest::GetCountersigningSessionState(Box::new(bob.cell_id.clone()));
    let response = bob.app_tx.request(request);
    let call_response = check_timeout(response, 6000).await.unwrap();
    match call_response {
        AppResponse::CountersigningSessionState(response) => {
            println!("res {:?}", response)
        }
        _ => panic!("unexpected zome call response"),
    }

    tokio::time::sleep(Duration::from_secs(10)).await;
    // await_consistency(30, vec![alice, bob, carol])
    //     .await
    //     .unwrap();

    // Need chain head for each other, so get agent activity before starting a session
    let _: AgentActivity = call_zome(
        &alice,
        "get_agent_activity",
        &GetAgentActivityInput {
            agent_pubkey: alice.cell_id.agent_pubkey().clone(),
            chain_query_filter: ChainQueryFilter::new(),
            activity_request: ActivityRequest::Full,
        },
    )
    .await;
    let _: AgentActivity = call_zome(
        &bob,
        "get_agent_activity",
        &GetAgentActivityInput {
            agent_pubkey: alice.cell_id.agent_pubkey().clone(),
            chain_query_filter: ChainQueryFilter::new(),
            activity_request: ActivityRequest::Full,
        },
    )
    .await;

    // Set up the session and accept it for both agents
    let preflight_request: PreflightRequest = call_zome(
        &alice,
        "generate_countersigning_preflight_request_fast",
        &[
            (alice.cell_id.agent_pubkey().clone(), vec![Role(0)]),
            (bob.cell_id.agent_pubkey().clone(), vec![]),
        ],
    )
    .await;
    let alice_acceptance: PreflightRequestAcceptance = call_zome(
        &alice,
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

    let request = AppRequest::GetCountersigningSessionState(Box::new(alice.cell_id.clone()));
    let response = alice.app_tx.request(request);
    let call_response = check_timeout(response, 6000).await.unwrap();
    match call_response {
        AppResponse::CountersigningSessionState(response) => {
            println!("res {:?}", response)
        }
        _ => panic!("unexpected zome call response"),
    }

    let result: ActionHash = call_zome(&bob, "create_a_thing", &()).await;
    println!("result is {result:?}");
    // let bob_acceptance: PreflightRequestAcceptance = conductors[1]
    //     .call_fallible(
    //         &bob_zome,
    //         "accept_countersigning_preflight_request",
    //         preflight_request.clone(),
    //     )
    //     .await
    //     .unwrap();
    // let bob_response = if let PreflightRequestAcceptance::Accepted(ref response) = bob_acceptance {
    //     response
    // } else {
    //     unreachable!();
    // };
}

struct Agent {
    app_tx: WebsocketSender,
    cell_id: CellId,
    signing_keypair: SigningKey,
    cap_secret: CapSecret,
    _holochain: SupervisedChild,
    _admin_rx: WsPollRecv,
    _app_rx: WsPollRecv,
}

async fn setup_agent(i: u16) -> Agent {
    let admin_port = i;
    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().to_path_buf();
    let environment_path = path.clone();
    let config = create_config(admin_port, environment_path.into());
    let config_path = write_config(path, &config);

    let (_holochain, admin_port) = start_holochain(config_path.clone()).await;
    let admin_port = admin_port.await.unwrap();

    let (mut admin_tx, admin_rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _admin_rx = WsPollRecv::new::<AdminResponse>(admin_rx);

    let uuid = uuid::Uuid::new_v4();
    let dna = fake_dna_zomes(
        &uuid.to_string(),
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

    // Attach App Interface
    let app_port = attach_app_interface(&mut admin_tx, None).await;

    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);
    authenticate_app_ws_client(app_tx.clone(), admin_port, "test".to_string()).await;

    Agent {
        app_tx,
        cell_id,
        signing_keypair,
        cap_secret,
        _admin_rx,
        _app_rx,
        _holochain,
    }
}

async fn call_zome<I, O>(agent: &Agent, fn_name: impl Into<FunctionName>, input: &I) -> O
where
    I: Serialize + std::fmt::Debug,
    O: DeserializeOwned + std::fmt::Debug,
{
    let zome_name = TestWasm::CounterSigning.coordinator_zome_name();
    call_zome_fn(
        &agent.app_tx,
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
