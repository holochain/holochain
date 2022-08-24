#![allow(deprecated)]

use ::fixt::prelude::*;
use hdk::prelude::*;

use holochain::conductor::api::AppInterfaceApi;
use holochain::conductor::api::AppRequest;
use holochain::conductor::api::AppResponse;
use holochain::conductor::api::ZomeCall;
use holochain::test_utils::setup_app;
use holochain_state::nonce::fresh_nonce;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_wasm_test_utils::TestZomes;
pub use holochain_zome_types::capability::CapSecret;

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
struct CreateMessageInput {
    channel_hash: EntryHash,
    content: String,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct ChannelName(String);

#[tokio::test(flavor = "multi_thread")]
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

#[tokio::test(flavor = "multi_thread")]
/// we can call a fn on a remote
async fn ser_regression_test() {
    observability::test_run().ok();
    let now = Timestamp::now();
    // ////////////
    // START DNA
    // ////////////

    let dna_file = DnaFile::new(
        DnaDef {
            name: "ser_regression_test".to_string(),
            network_seed: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            origin_time: Timestamp::HOLOCHAIN_EPOCH,
            integrity_zomes: vec![TestZomes::from(TestWasm::SerRegression)
                .integrity
                .into_inner()],
            coordinator_zomes: vec![TestZomes::from(TestWasm::SerRegression)
                .coordinator
                .into_inner()],
        },
        <Vec<DnaWasm>>::from(TestWasm::SerRegression),
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

    let (_tmpdir, app_api, handle) = setup_app(
        vec![dna_file],
        vec![(alice_installed_cell, None), (bob_installed_cell, None)],
    )
    .await;

    // /////////////
    // END CONDUCTOR
    // /////////////

    // ALICE DOING A CALL

    let channel = ChannelName("hello world".into());

    let (nonce, expires_at) = fresh_nonce(now).unwrap();
    let invocation = ZomeCall::try_from_unsigned_zome_call(
        handle.keystore(),
        ZomeCallUnsigned {
            cell_id: alice_cell_id.clone(),
            zome_name: TestWasm::SerRegression.into(),
            cap_secret: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
            fn_name: "create_channel".into(),
            payload: ExternIO::encode(channel).unwrap(),
            provenance: alice_agent_id.clone(),
            nonce,
            expires_at,
        },
    )
    .await
    .unwrap();

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
    let (nonce, expires_at) = fresh_nonce(now).unwrap();
    let invocation = ZomeCall::try_from_unsigned_zome_call(
        handle.keystore(),
        ZomeCallUnsigned {
            cell_id: alice_cell_id.clone(),
            zome_name: TestWasm::SerRegression.into(),
            cap_secret: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
            fn_name: "create_message".into(),
            payload: ExternIO::encode(message).unwrap(),
            provenance: alice_agent_id.clone(),
            nonce,
            expires_at,
        },
    )
    .await
    .unwrap();

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

    let shutdown = handle.take_shutdown_handle().unwrap();
    handle.shutdown();
    shutdown.await.unwrap().unwrap();
}
