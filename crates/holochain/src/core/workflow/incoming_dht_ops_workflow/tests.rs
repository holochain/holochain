use super::*;
use crate::conductor::space::TestSpace;
use ::fixt::prelude::*;
use holo_hash::fixt::DnaHashFixturator;
use holochain_keystore::test_keystore;
use holochain_keystore::AgentPubKeyExt;
use holochain_state::dht_store::DhtStore;

#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_to_limbo() {
    holochain_trace::test_run();

    let space = TestSpace::new(fixt!(DnaHash));
    let dht_store = space.space.dht_store.clone();
    let keystore = test_keystore();

    let author = fake_agent_pubkey_1();

    let mut hash_list = Vec::new();
    let mut op_list = Vec::new();

    for _ in 0..10 {
        let mut action = fixt!(CreateLink);
        action.author = author.clone();
        let action = Action::CreateLink(action);
        let signature = author.sign(&keystore, &action).await.unwrap();

        let op = ChainOp::RegisterAgentActivity(signature, action);
        let hash = DhtOpHash::with_data_sync(&op);
        hash_list.push(hash);
        op_list.push(op);
    }

    let mut all = Vec::new();
    for op in op_list {
        let (sys_validation_trigger, _) = TriggerSender::new();
        let space = space.space.clone();
        all.push(tokio::task::spawn(async move {
            let start = std::time::Instant::now();
            incoming_dht_ops_workflow(space, sys_validation_trigger, vec![op.into()])
                .await
                .unwrap();
            println!("IN OP in {} s", start.elapsed().as_secs_f64());
        }));
    }

    futures::future::try_join_all(all).await.unwrap();

    verify_ops_present(&dht_store, hash_list, true).await;
}

// Checks that there is no other record of the op hash being held onto outside of the database that will prevent
// reprocessing.
#[tokio::test(flavor = "multi_thread")]
async fn can_retry_failed_op() {
    holochain_trace::test_run();

    let space = TestSpace::new(fixt!(DnaHash));
    let dht_store = space.space.dht_store.clone();
    let keystore = test_keystore();
    let (sys_validation_trigger, mut sys_validation_rx) = TriggerSender::new();

    let author = keystore.new_sign_keypair_random().await.unwrap();

    let mut action = fixt!(CreateLink);
    action.author = author.clone();
    let action = Action::CreateLink(action);
    // Create a dummy signature that will fail validation
    let signature = Signature([0; SIGNATURE_BYTES]);

    let op = ChainOp::RegisterAgentActivity(signature, action.clone()).into();
    let hash = DhtOpHash::with_data_sync(&op);

    // Try running the workflow and...
    let workflow_result = incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger.clone(),
        vec![op],
    )
    .await;

    // .. check that the workflow failed, with the ops NOT saved to the database
    assert!(workflow_result.is_err());
    verify_ops_present(&dht_store, vec![hash], false).await;

    // Now fix the signature
    let signature = author.sign(&keystore, &action).await.unwrap();
    let op = ChainOp::RegisterAgentActivity(signature, action).into();
    let hash = DhtOpHash::with_data_sync(&op);

    // Run the workflow again to simulate a re-send of the op...
    incoming_dht_ops_workflow(space.space.clone(), sys_validation_trigger, vec![op])
        .await
        .unwrap();

    // ... and now it should succeed
    verify_is_pending_validation_receipt(&dht_store, hash).await;
    // then sys validation should run on the new op
    sys_validation_rx.listen().await.unwrap();
    // and no ops should be claimed for processing
    assert!(space.space.incoming_op_hashes.0.lock().is_empty());
}

async fn verify_is_pending_validation_receipt(dht_store: &DhtStore, hash: DhtOpHash) {
    let pending_hashes = get_pending_op_hashes(dht_store).await;

    tracing::info!("Found {} ops", pending_hashes.len());

    assert!(pending_hashes
        .into_iter()
        .any(|pending_hash| pending_hash == hash));
}

async fn verify_ops_present(dht_store: &DhtStore, hash_list: Vec<DhtOpHash>, present: bool) {
    for hash in hash_list {
        let found = dht_store.as_read().limbo_op_exists(&hash).await.unwrap();
        assert_eq!(present, found);
    }
}

async fn get_pending_op_hashes(dht_store: &DhtStore) -> Vec<DhtOpHash> {
    dht_store
        .as_read()
        .limbo_op_hashes_requiring_receipt()
        .await
        .unwrap()
}
