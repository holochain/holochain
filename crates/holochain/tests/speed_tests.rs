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

#![allow(deprecated)]

use ::fixt::prelude::*;
use hdk::prelude::*;
use holochain::conductor::api::AdminRequest;
use holochain::conductor::api::AdminResponse;
use holochain::conductor::api::AppRequest;
use holochain::conductor::api::AppResponse;
use holochain::conductor::api::RealAppInterfaceApi;
use holochain::conductor::api::ZomeCall;
use holochain::conductor::config::AdminInterfaceConfig;
use holochain::conductor::config::ConductorConfig;
use holochain::conductor::config::InterfaceDriver;
use holochain::conductor::ConductorBuilder;
use holochain::conductor::ConductorHandle;

use holochain_sqlite::test_utils::test_environments;
use holochain_sqlite::test_utils::TestDbs;
use holochain_test_wasm_common::AnchorInput;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::WebsocketResult;
use holochain_websocket::WebsocketSender;
use matches::assert_matches;
use observability;
use test_case::test_case;
use test_utils::*;
use tracing::instrument;

mod test_utils;

const DEFAULT_NUM: usize = 2000;

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
#[ignore = "speed tests are ignored by default; unignore to run"]
async fn speed_test_prep() {
    holochain::test_utils::warm_wasm_tests();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "speed tests are ignored by default; unignore to run"]
async fn speed_test_timed() {
    let _g = observability::test_run_timed().unwrap();
    speed_test(None).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "speed tests are ignored by default; unignore to run"]
async fn speed_test_timed_json() {
    let _g = observability::test_run_timed_json().unwrap();
    speed_test(None).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "speed tests are ignored by default; unignore to run"]
async fn speed_test_timed_flame() {
    let _g = observability::test_run_timed_flame(None).unwrap();
    speed_test(None).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "speed tests are ignored by default; unignore to run"]
async fn speed_test_timed_ice() {
    let _g = observability::test_run_timed_ice(None).unwrap();
    speed_test(None).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "speed tests are ignored by default; unignore to run"]
async fn speed_test_normal() {
    observability::test_run().unwrap();
    speed_test(None).await;
}

/// Run this test to execute the speed test, but then keep the LMDB env files
/// around in temp dirs for inspection by e.g. `mdb_stat`
#[tokio::test(flavor = "multi_thread")]
#[ignore = "speed tests are ignored by default; unignore to run"]
async fn speed_test_persisted() {
    observability::test_run().unwrap();
    let envs = speed_test(None).await;
    let tmpdir = envs.tempdir();
    drop(envs);
    let tmpdir = std::sync::Arc::try_unwrap(tmpdir).unwrap();
    let path = tmpdir.into_path();
    println!("Run the following to see info about the test that just ran,");
    println!("with the correct cell env dir appended to the path:");
    println!();
    println!("    $ mdb_stat -afe {}/", path.to_string_lossy());
    println!();
}

#[test_case(1)]
#[test_case(10)]
#[test_case(100)]
#[test_case(1000)]
#[test_case(2000)]
#[ignore = "speed tests are ignored by default; unignore to run"]
fn speed_test_all(n: usize) {
    observability::test_run().unwrap();
    tokio_helper::block_forever_on(speed_test(Some(n)));
}

#[instrument]
async fn speed_test(n: Option<usize>) -> TestDbs {
    let num = n.unwrap_or(DEFAULT_NUM);

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
    dna_store.expect_get_entry_def().return_const(None);

    let (test_env, _app_api, handle) = setup_app(
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

    fn new_zome_call<P>(
        cell_id: CellId,
        func: &str,
        payload: P,
    ) -> Result<ZomeCall, SerializedBytesError>
    where
        P: serde::Serialize + std::fmt::Debug,
    {
        Ok(ZomeCall {
            cell_id: cell_id.clone(),
            zome_name: TestWasm::Anchor.into(),
            cap: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
            fn_name: func.into(),
            payload: ExternIO::encode(payload)?,
            provenance: cell_id.agent_pubkey().clone(),
        })
    }

    let anchor_invocation = |anchor: &str, cell_id, i: usize| {
        let anchor = AnchorInput(anchor.into(), i.to_string());
        new_zome_call(cell_id, "anchor", anchor)
    };

    async fn call(
        app_interface: &mut WebsocketSender,
        invocation: ZomeCall,
    ) -> WebsocketResult<AppResponse> {
        let request = AppRequest::ZomeCall(Box::new(invocation));
        app_interface.request(request).await
    }

    let timer = std::time::Instant::now();

    for i in 0..num {
        let invocation = anchor_invocation("alice", alice_cell_id.clone(), i).unwrap();
        let response = call(&mut app_interface, invocation).await.unwrap();
        assert_matches!(response, AppResponse::ZomeCall(_));
        let invocation = anchor_invocation("bobbo", bob_cell_id.clone(), i).unwrap();
        let response = call(&mut app_interface, invocation).await.unwrap();
        assert_matches!(response, AppResponse::ZomeCall(_));
    }

    let mut alice_done = false;
    let mut bobbo_done = false;
    let mut alice_attempts = 0_usize;
    let mut bobbo_attempts = 0_usize;
    loop {
        if !bobbo_done {
            bobbo_attempts += 1;
            let invocation = new_zome_call(
                alice_cell_id.clone(),
                "list_anchor_addresses",
                "bobbo".to_string(),
            )
            .unwrap();
            let response = call(&mut app_interface, invocation).await.unwrap();
            let hashes: EntryHashes = match response {
                AppResponse::ZomeCall(r) => r.decode().unwrap(),
                _ => unreachable!(),
            };
            bobbo_done = hashes.0.len() == num;
        }

        if !alice_done {
            alice_attempts += 1;
            let invocation = new_zome_call(
                bob_cell_id.clone(),
                "list_anchor_addresses",
                "alice".to_string(),
            )
            .unwrap();
            let response = call(&mut app_interface, invocation).await.unwrap();
            let hashes: EntryHashes = match response {
                AppResponse::ZomeCall(r) => r.decode().unwrap(),
                _ => unreachable!(),
            };
            alice_done = hashes.0.len() == num;
        }
        if alice_done && bobbo_done {
            let el = timer.elapsed();
            println!(
                "Consistency in for {} calls: {}ms or {}s\n
                Alice took {} attempts to reach consistency\n
                Bobbo took {} attempts to reach consistency",
                num,
                el.as_millis(),
                el.as_secs(),
                alice_attempts,
                bobbo_attempts,
            );
            break;
        }
    }
    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
    test_env
}

pub async fn setup_app(
    cell_data: Vec<(InstalledCell, Option<SerializedBytes>)>,
    dna_store: MockDnaStore,
) -> (TestDbs, RealAppInterfaceApi, ConductorHandle) {
    let envs = test_environments();

    let conductor_handle = ConductorBuilder::with_mock_dna_store(dna_store)
        .config(ConductorConfig {
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        })
        .test(&envs)
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

    (
        envs,
        RealAppInterfaceApi::new(conductor_handle, Default::default()),
        handle,
    )
}
