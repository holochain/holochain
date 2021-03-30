use super::*;
use crate::conductor::ConductorHandle;
use crate::test_utils::setup_app;
use crate::test_utils::wait_for_integration;
use ::fixt::prelude::*;
use holochain_keystore::AgentPubKeyExt;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::source_chain::SourceChain;

use holochain_wasm_test_utils::TestWasm;

use std::convert::TryFrom;
use std::time::Duration;

/// Unfortunately this test doesn't do anything yet because
/// failing a chain validation is just a log error so the only way to
/// verify this works is to run this with logging and check it outputs
/// use `RUST_LOG=[agent_activity]=warn`
#[tokio::test(flavor = "multi_thread")]
#[ignore = "TODO: complete when chain validation returns actual error"]
async fn sys_validation_agent_activity_test() {
    observability::test_run().ok();

    let dna_file = DnaFile::new(
        DnaDef {
            name: "chain_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::Create.into()].into(),
        },
        vec![TestWasm::Create.into()],
    )
    .await
    .unwrap();

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
    let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![("test_app", vec![(alice_installed_cell, None)])],
        vec![dna_file.clone()],
    )
    .await;

    run_test(alice_cell_id, handle.clone()).await;

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap().unwrap();
}

async fn run_test(alice_cell_id: CellId, handle: ConductorHandle) {
    // Setup
    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    let alice_triggers = handle.get_cell_triggers(&alice_cell_id).await.unwrap();
    let sys_validation_trigger = alice_triggers.sys_validation;

    // Wait for genesis to integrate
    wait_for_integration(&alice_env, 7, 100, Duration::from_millis(100)).await;

    let source_chain = SourceChain::new(alice_env.clone().into()).unwrap();
    let mut timestamp = timestamp::now();

    // Create the headers
    let mut h1 = fixt!(Create);
    let mut h2 = fixt!(Create);

    // Set correct agent keys
    h1.author = alice_cell_id.agent_pubkey().clone();
    h2.author = alice_cell_id.agent_pubkey().clone();

    // Set valid prev header
    h1.prev_header = source_chain
        .get_at_index(2)
        .unwrap()
        .unwrap()
        .header_address()
        .clone();

    // Set valid timestamps
    h1.timestamp = timestamp.clone().into();
    timestamp.0 += 1;
    h2.timestamp = timestamp.clone().into();

    // Set valid header seq
    h1.header_seq = 3;
    h2.header_seq = 4;

    // Set valid prev header
    h2.prev_header = HeaderHash::with_data_sync(&Header::Create(h1.clone()));

    let mut ops = vec![];

    // Make valid signature
    let signature = alice_cell_id
        .agent_pubkey()
        .sign(&alice_env.keystore(), &Header::Create(h1.clone()))
        .await
        .unwrap();

    // Create the activity op
    let op = DhtOp::RegisterAgentActivity(signature, h1.clone().into());
    ops.push((DhtOpHash::with_data_sync(&op), op));

    // Make valid signature
    let signature = alice_cell_id
        .agent_pubkey()
        .sign(&alice_env.keystore(), &Header::Create(h2.clone()))
        .await
        .unwrap();

    // Create the activity op
    let op = DhtOp::RegisterAgentActivity(signature, h2.clone().into());
    ops.push((DhtOpHash::with_data_sync(&op), op));

    // Add the ops to incoming
    incoming_dht_ops_workflow::incoming_dht_ops_workflow(
        &alice_env,
        sys_validation_trigger.clone(),
        ops,
        None,
        false,
    )
    .await
    .unwrap();

    wait_for_integration(&alice_env, 7 + 2, 100, Duration::from_millis(100)).await;

    // Check you don't see any warning output
    // TODO: When we add invalid chains put a real check here

    // set valid prev header chain
    let last_hash = HeaderHash::with_data_sync(&Header::Create(h2.clone()));
    h1.prev_header = last_hash.clone();

    // set valid timestamps
    timestamp.0 += 1;
    h1.timestamp = timestamp.clone().into();
    timestamp.0 += 1;
    h2.timestamp = timestamp.clone().into();

    // Create a chain fork
    h1.header_seq = 5;
    h2.header_seq = 5;

    // Set valid prev header
    h2.prev_header = last_hash;

    let mut ops = vec![];

    // Make valid signature
    let signature = alice_cell_id
        .agent_pubkey()
        .sign(&alice_env.keystore(), &Header::Create(h1.clone()))
        .await
        .unwrap();

    // Create the activity op
    let op = DhtOp::RegisterAgentActivity(signature, h1.into());
    ops.push((DhtOpHash::with_data_sync(&op), op));

    // Make valid signature
    let signature = alice_cell_id
        .agent_pubkey()
        .sign(&alice_env.keystore(), &Header::Create(h2.clone()))
        .await
        .unwrap();

    // Create the activity op
    let op = DhtOp::RegisterAgentActivity(signature, h2.into());
    ops.push((DhtOpHash::with_data_sync(&op), op));

    // Add the ops to incoming
    incoming_dht_ops_workflow::incoming_dht_ops_workflow(
        &alice_env,
        sys_validation_trigger,
        ops,
        None,
        false,
    )
    .await
    .unwrap();

    wait_for_integration(&alice_env, 9 + 2, 100, Duration::from_millis(100)).await;

    // Check you **do** see any warning output
    // TODO: When we add invalid chains put a real check here
}
