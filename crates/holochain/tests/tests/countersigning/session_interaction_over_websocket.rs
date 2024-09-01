//! Test countersigning session interaction with full Holochain conductor over websockets.
//!
//! Tests run the Holochain binary and communicate over websockets.

use std::time::Duration;

use ed25519_dalek::SigningKey;
use hdk::prelude::{CapSecret, CellId, FunctionName, ZomeName};
use holo_hash::AgentPubKey;
use holochain::sweettest::{authenticate_app_ws_client, websocket_client_by_port, WsPollRecv};
use holochain_conductor_api::{AdminRequest, AdminResponse, AppResponse};
use holochain_types::test_utils::{fake_dna_zomes, write_fake_dna_file};
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::WebsocketSender;
use matches::assert_matches;
use rand::rngs::OsRng;
use serde::{de::DeserializeOwned, Serialize};
use tempfile::TempDir;

use crate::tests::test_utils::{
    attach_app_interface, call_zome_fn, check_timeout, create_config, grant_zome_call_capability,
    register_and_install_dna, start_holochain, write_config,
};

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn get_session_state() {
    use std::collections::BTreeSet;

    use arbitrary::Arbitrary;
    use ed25519_dalek::ed25519::signature::SignerMut;
    use hdk::prelude::{
        CapAccess, CapSecret, ExternIO, GrantZomeCallCapabilityPayload, GrantedFunctions,
        Signature, Timestamp, ZomeCallCapGrant, ZomeCallUnsigned,
    };
    use holo_hash::ActionHash;
    use holochain_conductor_api::{AppRequest, ZomeCall};

    holochain_trace::test_run();

    let admin_port = 0;
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

    println!("app enabled");

    // Generate signing key pair
    let mut rng = OsRng;
    let mut signing_keypair = ed25519_dalek::SigningKey::generate(&mut rng);
    let signing_key = AgentPubKey::from_raw_32(signing_keypair.verifying_key().as_bytes().to_vec());

    // Grant zome call capability for agent
    let zome_name = TestWasm::CounterSigning.coordinator_zome_name();
    let functions = GrantedFunctions::All;

    let mut buf = arbitrary::Unstructured::new(&[]);
    let cap_secret = CapSecret::arbitrary(&mut buf).unwrap();

    let mut assignees = BTreeSet::new();
    assignees.insert(signing_key.clone());

    println!("granting zome call capability");
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

    println!("yes, that worked");

    // Attach App Interface
    let app_port = attach_app_interface(&mut admin_tx, None).await;

    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);
    authenticate_app_ws_client(app_tx.clone(), admin_port, "test".to_string()).await;

    // Call Zome
    let result: ActionHash = call_zome_fn(
        &app_tx,
        cell_id.clone(),
        &signing_keypair,
        cap_secret.clone(),
        zome_name.clone(),
        "create_a_thing".into(),
        &(),
    )
    .await;
    println!("result is {result:?}");

    drop(app_tx);
    drop(admin_tx);
}
