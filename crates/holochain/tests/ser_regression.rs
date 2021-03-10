#![allow(deprecated)]

use ::fixt::prelude::*;
use hdk::prelude::*;

use holochain::conductor::api::AppInterfaceApi;
use holochain::conductor::api::AppRequest;
use holochain::conductor::api::AppResponse;
use holochain::conductor::api::RealAppInterfaceApi;
use holochain::conductor::api::ZomeCall;
use holochain::conductor::ConductorBuilder;
use holochain::conductor::ConductorHandle;

use holochain_lmdb::test_utils::test_environments;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
pub use holochain_zome_types::capability::CapSecret;
use observability;
use std::sync::Arc;
use tempdir::TempDir;

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
struct CreateMessageInput {
    channel_hash: EntryHash,
    content: String,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct ChannelName(String);

#[tokio::test(threaded_scheduler)]
async fn ser_entry_hash_test() {
    observability::test_run().ok();
    let eh = fixt!(EntryHash);
    let extern_io: ExternIO = ExternIO::encode(eh).unwrap();
    tracing::debug!(?extern_io);
    let o: EntryHash = extern_io.decode().unwrap();
    let extern_io: ExternIO = ExternIO::encode(o).unwrap();
    tracing::debug!(?extern_io);
    let _eh: EntryHash = extern_io.decode().unwrap();
}

#[tokio::test(threaded_scheduler)]
/// we can call a fn on a remote
async fn ser_regression_test() {
    observability::test_run().ok();
    // ////////////
    // START DNA
    // ////////////

    let dna_file = DnaFile::new(
        DnaDef {
            name: "ser_regression_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::SerRegression.into()].into(),
        },
        vec![TestWasm::SerRegression.into()],
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

    let (_tmpdir, app_api, handle) = setup_app(
        vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        dna_store,
    )
    .await;

    // /////////////
    // END CONDUCTOR
    // /////////////

    // ALICE DOING A CALL

    let channel = ChannelName("hello world".into());

    let invocation = ZomeCall {
        cell_id: alice_cell_id.clone(),
        zome_name: TestWasm::SerRegression.into(),
        cap: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
        fn_name: "create_channel".into(),
        payload: ExternIO::encode(channel).unwrap(),
        provenance: alice_agent_id.clone(),
    };

    let request = Box::new(invocation.clone());
    let request = AppRequest::ZomeCall(request).try_into().unwrap();
    let response = app_api.handle_app_request(request).await;

    let _channel_hash: EntryHash = match response {
        AppResponse::ZomeCall(r) => r.decode().unwrap(),
        _ => unreachable!(),
    };

    let output = handle.call_zome(invocation).await.unwrap().unwrap();

    let channel_hash: EntryHash = match output {
        ZomeCallResponse::Ok(guest_output) => guest_output.decode().unwrap(),
        _ => unreachable!(),
    };

    let message = CreateMessageInput {
        channel_hash,
        content: "Hello from alice :)".into(),
    };
    let invocation = ZomeCall {
        cell_id: alice_cell_id.clone(),
        zome_name: TestWasm::SerRegression.into(),
        cap: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
        fn_name: "create_message".into(),
        payload: ExternIO::encode(message).unwrap(),
        provenance: alice_agent_id.clone(),
    };

    let request = Box::new(invocation.clone());
    let request = AppRequest::ZomeCall(request).try_into().unwrap();
    let response = app_api.handle_app_request(request).await;

    let _msg_hash: EntryHash = match response {
        AppResponse::ZomeCall(r) => r.decode().unwrap(),
        _ => unreachable!(),
    };

    let output = handle.call_zome(invocation).await.unwrap().unwrap();

    let _msg_hash: EntryHash = match output {
        ZomeCallResponse::Ok(guest_output) => guest_output.decode().unwrap(),
        _ => unreachable!(),
    };

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

pub async fn setup_app(
    cell_data: Vec<(InstalledCell, Option<SerializedBytes>)>,
    dna_store: MockDnaStore,
) -> (Arc<TempDir>, RealAppInterfaceApi, ConductorHandle) {
    let envs = test_environments();
    let conductor_handle = ConductorBuilder::with_mock_dna_store(dna_store)
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
        envs.tempdir(),
        RealAppInterfaceApi::new(conductor_handle, Default::default()),
        handle,
    )
}
