//! # Speed tests
//! These are designed to diagnose performance issues from a macro level.
//! They are not intended to detect performance regressions or to be run in CI.
//! For that a latency test or benchmark should be used.
//! These tests are useful once you know there is an issue in locating which
//! part of the codebase it is.
//! An example of running the flame test to produce a flamegraph is:
//! ```fish
//! env RUST_LOG='[{}]=debug' HC_WASM_CACHE_PATH='/path/to/holochain/.wasm_cache' \
//! cargo test  --release --quiet --test speed_tests \
//! --  --nocapture --ignored --exact --test speed_test_timed_flame \
//! 2>| inferno-flamegraph > flamegraph_test_ice_(date +'%d-%m-%y-%X').svg
//! ```
//! I plan to make this all automatic as a single command in the future but it's
//! hard to automate piping from tests stderr.
//!

use hdk3::prelude::*;
use holochain::conductor::{
    api::{AdminRequest, AdminResponse, AppRequest, AppResponse, RealAppInterfaceApi},
    config::{AdminInterfaceConfig, ConductorConfig, InterfaceDriver},
    dna_store::MockDnaStore,
    ConductorBuilder, ConductorHandle,
};
use holochain::core::ribosome::ZomeCallInvocation;
use holochain_state::test_utils::{test_conductor_env, test_wasm_env, TestEnvironment};
use holochain_types::app::InstalledCell;
use holochain_types::cell::CellId;
use holochain_types::dna::DnaDef;
use holochain_types::dna::DnaFile;
use holochain_types::test_utils::fake_agent_pubkey_1;
use holochain_types::{observability, test_utils::fake_agent_pubkey_2};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::HostInput;
use std::sync::Arc;
use tempdir::TempDir;

use holochain_websocket::WebsocketSender;
use matches::assert_matches;
use test_utils::*;
use test_wasm_common::{AnchorInput, TestString};
use tracing::instrument;

mod test_utils;

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn speed_test_flame() {
    let _g = observability::flame_run().unwrap();
    let _g = _g.unwrap();
    speed_test().await;
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn speed_test_timed() {
    let _g = observability::test_run_timed().unwrap();
    speed_test().await;
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn speed_test_timed_json() {
    let _g = observability::test_run_timed_json().unwrap();
    speed_test().await;
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn speed_test_timed_flame() {
    let _g = observability::test_run_timed_flame(None).unwrap();
    speed_test().await;
    tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn speed_test_timed_ice() {
    let _g = observability::test_run_timed_ice(None).unwrap();
    speed_test().await;
    tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn speed_test_normal() {
    observability::test_run().unwrap();
    speed_test().await;
}

#[instrument]
async fn speed_test() {
    const NUM: usize = 2000;

    // ////////////
    // START DNA
    // ////////////

    let dna_file = DnaFile::new(
        DnaDef {
            name: "need_for_speed_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::Anchor.into()].into(),
        },
        vec![TestWasm::Anchor.into()],
    )
    .await
    .unwrap();

    // //////////
    // END DNA
    // //////////

    // ///////////
    // START ALICE
    // ///////////

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
    let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

    // /////////
    // END ALICE
    // /////////

    // /////////
    // START BOB
    // /////////

    let bob_agent_id = fake_agent_pubkey_2();
    let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
    let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());

    // ///////
    // END BOB
    // ///////

    // ///////////////
    // START CONDUCTOR
    // ///////////////

    let mut dna_store = MockDnaStore::new();

    dna_store.expect_get().return_const(Some(dna_file.clone()));
    dna_store
        .expect_add_dnas::<Vec<_>>()
        .times(2)
        .return_const(());
    dna_store
        .expect_add_entry_defs::<Vec<_>>()
        .times(2)
        .return_const(());

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        dna_store,
    )
    .await;

    // Setup websocket handle and app interface
    let (mut client, _) = websocket_client(&handle).await.unwrap();
    let request = AdminRequest::AttachAppInterface { port: None };
    let response = client.request(request);
    let response = response.await.unwrap();
    let app_port = match response {
        AdminResponse::AppInterfaceAttached { port } => port,
        _ => panic!("Attach app interface failed: {:?}", response),
    };
    let (mut app_interface, _) = websocket_client_by_port(app_port).await.unwrap();

    // /////////////
    // END CONDUCTOR
    // /////////////

    // ALICE DOING A CALL

    fn new_invocation<P>(
        cell_id: CellId,
        func: &str,
        payload: P,
    ) -> Result<ZomeCallInvocation, SerializedBytesError>
    where
        P: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        Ok(ZomeCallInvocation {
            cell_id: cell_id.clone(),
            zome_name: TestWasm::Anchor.into(),
            cap: CapSecret::default(),
            fn_name: func.to_string(),
            payload: HostInput::new(payload.try_into()?),
            provenance: cell_id.agent_pubkey().clone(),
        })
    }

    let anchor_invocation = |anchor: &str, cell_id, i: usize| {
        let anchor = AnchorInput(anchor.into(), i.to_string());
        new_invocation(cell_id, "anchor", anchor)
    };

    async fn call(
        app_interface: &mut WebsocketSender,
        invocation: ZomeCallInvocation,
    ) -> std::io::Result<AppResponse> {
        let request = AppRequest::ZomeCallInvocation(Box::new(invocation));
        app_interface.request(request).await
    }

    for i in 0..NUM {
        let invocation = anchor_invocation("alice", alice_cell_id.clone(), i).unwrap();
        let response = call(&mut app_interface, invocation).await.unwrap();
        assert_matches!(response, AppResponse::ZomeCallInvocation(_));
        let invocation = anchor_invocation("bobbo", bob_cell_id.clone(), i).unwrap();
        let response = call(&mut app_interface, invocation).await.unwrap();
        assert_matches!(response, AppResponse::ZomeCallInvocation(_));
    }

    // Give a little time for gossip to process
    tokio::time::delay_for(std::time::Duration::from_millis(100)).await;

    let invocation = new_invocation(
        alice_cell_id.clone(),
        "list_anchor_addresses",
        TestString("bobbo".into()),
    )
    .unwrap();
    let response = call(&mut app_interface, invocation).await.unwrap();
    match response {
        AppResponse::ZomeCallInvocation(r) => {
            let response: SerializedBytes = r.into_inner();
            let hashes: EntryHashes = response.try_into().unwrap();
            assert_eq!(hashes.0.len(), NUM);
        }
        _ => unreachable!(),
    }

    let invocation = new_invocation(
        bob_cell_id.clone(),
        "list_anchor_addresses",
        TestString("alice".into()),
    )
    .unwrap();
    let response = call(&mut app_interface, invocation).await.unwrap();
    match response {
        AppResponse::ZomeCallInvocation(r) => {
            let response: SerializedBytes = r.into_inner();
            let hashes: EntryHashes = response.try_into().unwrap();
            assert_eq!(hashes.0.len(), NUM);
        }
        _ => unreachable!(),
    }

    app_interface
        .close(1000, "Shutting down".into())
        .await
        .unwrap();
    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

pub async fn setup_app(
    cell_data: Vec<(InstalledCell, Option<SerializedBytes>)>,
    dna_store: MockDnaStore,
) -> (Arc<TempDir>, RealAppInterfaceApi, ConductorHandle) {
    let test_env = test_conductor_env();
    let TestEnvironment {
        env: wasm_env,
        tmpdir: _tmpdir,
    } = test_wasm_env();
    let tmpdir = test_env.tmpdir.clone();

    let conductor_handle = ConductorBuilder::with_mock_dna_store(dna_store)
        .config(ConductorConfig {
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        })
        .test(test_env, wasm_env)
        .await
        .unwrap();

    conductor_handle
        .clone()
        .install_app("test app".to_string(), cell_data)
        .await
        .unwrap();

    conductor_handle
        .activate_app("test app".to_string())
        .await
        .unwrap();

    let errors = conductor_handle.clone().setup_cells().await.unwrap();

    assert!(errors.is_empty());

    let handle = conductor_handle.clone();

    (tmpdir, RealAppInterfaceApi::new(conductor_handle), handle)
}
