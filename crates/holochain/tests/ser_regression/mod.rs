use ::fixt::prelude::*;
use hdk::prelude::*;

use holochain::conductor::api::AppInterfaceApi;
use holochain::conductor::api::AppRequest;
use holochain::conductor::api::AppResponse;
use holochain::conductor::api::RealAppInterfaceApi;
use holochain::conductor::api::ZomeCall;
use holochain::sweettest::*;
use holochain_nonce::fresh_nonce;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_wasm_test_utils::TestZomes;

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
struct CreateMessageInput {
    channel_hash: EntryHash,
    content: String,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct ChannelName(String);

#[tokio::test(flavor = "multi_thread")]
async fn ser_entry_hash_test() {
    holochain_trace::test_run().ok();
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
    holochain_trace::test_run().ok();
    // ////////////
    // START DNA
    // ////////////

    let dna_file = DnaFile::new(
        DnaDef {
            name: "ser_regression_test".to_string(),
            modifiers: DnaModifiers {
                network_seed: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
                properties: SerializedBytes::try_from(()).unwrap(),
                origin_time: Timestamp::HOLOCHAIN_EPOCH,
                quantum_time: holochain_p2p::dht::spacetime::STANDARD_QUANTUM_TIME,
            },
            integrity_zomes: vec![TestZomes::from(TestWasm::SerRegression)
                .integrity
                .into_inner()],
            coordinator_zomes: vec![TestZomes::from(TestWasm::SerRegression)
                .coordinator
                .into_inner()],
        },
        <Vec<DnaWasm>>::from(TestWasm::SerRegression),
    )
    .await;

    // //////////
    // END DNA
    // //////////

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;

    let installed_app_id = "app".to_string();
    let ((alice,), (_bob,)) = conductors
        .setup_app(&installed_app_id, vec![&dna_file])
        .await
        .unwrap()
        .into_tuples();

    // ALICE DOING A CALL

    let channel = ChannelName("hello world".into());

    let (nonce, expires_at) = fresh_nonce(Timestamp::now()).unwrap();
    let mut invocation = ZomeCall::try_from_unsigned_zome_call(
        &conductors[0].keystore(),
        ZomeCallUnsigned {
            cell_id: alice.cell_id().clone(),
            zome_name: TestWasm::SerRegression.into(),
            cap_secret: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
            fn_name: "create_channel".into(),
            payload: ExternIO::encode(channel).unwrap(),
            provenance: alice.agent_pubkey().clone(),
            nonce,
            expires_at,
        },
    )
    .await
    .unwrap();

    let app_api = RealAppInterfaceApi::new(conductors[0].clone(), installed_app_id);
    let request = Box::new(invocation.clone());
    let request = AppRequest::CallZome(request).try_into().unwrap();
    let response = app_api.handle_app_request(request).await;

    let _channel_hash: EntryHash = match response {
        AppResponse::ZomeCalled(r) => r.decode().unwrap(),
        _ => unreachable!(),
    };

    let (nonce, expires_at) = fresh_nonce(Timestamp::now()).unwrap();
    invocation.nonce = nonce;
    invocation.expires_at = expires_at;
    let invocation = invocation
        .resign_zome_call(&conductors[0].keystore(), alice.agent_pubkey().clone())
        .await
        .unwrap();
    let output = conductors[0].call_zome(invocation).await.unwrap().unwrap();

    let channel_hash: EntryHash = match output {
        ZomeCallResponse::Ok(guest_output) => guest_output.decode().unwrap(),
        _ => panic!("{:?}", output),
    };

    let message = CreateMessageInput {
        channel_hash,
        content: "Hello from alice :)".into(),
    };
    let (nonce, expires_at) = fresh_nonce(Timestamp::now()).unwrap();
    let mut invocation = ZomeCall::try_from_unsigned_zome_call(
        &conductors[0].keystore(),
        ZomeCallUnsigned {
            cell_id: alice.cell_id().clone(),
            zome_name: TestWasm::SerRegression.into(),
            cap_secret: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
            fn_name: "create_message".into(),
            payload: ExternIO::encode(message).unwrap(),
            provenance: alice.agent_pubkey().clone(),
            nonce,
            expires_at,
        },
    )
    .await
    .unwrap();

    let request = Box::new(invocation.clone());
    let request = AppRequest::CallZome(request).try_into().unwrap();
    let response = app_api.handle_app_request(request).await;

    let _msg_hash: EntryHash = match response {
        AppResponse::ZomeCalled(r) => r.decode().unwrap(),
        _ => unreachable!(),
    };

    let (nonce, expires_at) = fresh_nonce(Timestamp::now()).unwrap();
    invocation.nonce = nonce;
    invocation.expires_at = expires_at;
    let invocation = invocation
        .resign_zome_call(&conductors[0].keystore(), alice.agent_pubkey().clone())
        .await
        .unwrap();
    let output = conductors[0].call_zome(invocation).await.unwrap().unwrap();

    let _msg_hash: EntryHash = match output {
        ZomeCallResponse::Ok(guest_output) => guest_output.decode().unwrap(),
        _ => panic!("{:?}", output),
    };
}
