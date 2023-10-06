use crate::conductor::space::TestSpace;

use super::*;
use ::fixt::prelude::*;
use holochain_keystore::AgentPubKeyExt;
use holochain_state::test_utils::test_keystore;

#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_to_limbo() {
    holochain_trace::test_run().unwrap();

    let space = TestSpace::new(fixt!(DnaHash));
    let env = space.space.dht_db.clone();
    let keystore = test_keystore();

    let author = fake_agent_pubkey_1();

    let mut hash_list = Vec::new();
    let mut op_list = Vec::new();

    for _ in 0..10 {
        let mut action = fixt!(CreateLink);
        action.author = author.clone();
        let action = Action::CreateLink(action);
        let signature = author.sign(&keystore, &action).await.unwrap();

        let op = DhtOp::RegisterAgentActivity(signature, action);
        let hash = DhtOpHash::with_data_sync(&op);
        hash_list.push(hash.clone());
        op_list.push((hash, op));
    }

    let mut all = Vec::new();
    for op in op_list {
        let (sys_validation_trigger, _) = TriggerSender::new();
        let space = space.space.clone();
        all.push(tokio::task::spawn(async move {
            let start = std::time::Instant::now();
            incoming_dht_ops_workflow(space, sys_validation_trigger, vec![op], false)
                .await
                .unwrap();
            println!("IN OP in {} s", start.elapsed().as_secs_f64());
        }));
    }

    futures::future::try_join_all(all).await.unwrap();

    verify_ops_present(env, hash_list, true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn can_retry_failed_op() {
    holochain_trace::test_run().unwrap();

    let space = TestSpace::new(fixt!(DnaHash));
    let env = space.space.dht_db.clone();
    let keystore = test_keystore();
    let (sys_validation_trigger, mut sys_validation_rx) = TriggerSender::new();

    let author = keystore.new_sign_keypair_random().await.unwrap();

    let mut action = fixt!(CreateLink);
    action.author = author.clone();
    let action = Action::CreateLink(action);
    // Create a dummy signature that will fail validation
    let signature = Signature([0; SIGNATURE_BYTES]);

    let op = DhtOp::RegisterAgentActivity(signature, action.clone());
    let hash = DhtOpHash::with_data_sync(&op);

    // Try running the workflow and...
    let workflow_result = incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger.clone(),
        vec![(hash.clone(), op)],
        true,
    )
    .await;

    // .. check that the workflow failed, with the ops NOT saved to the database
    assert!(workflow_result.is_err());
    verify_ops_present(env.clone(), vec![hash], false).await;

    // Now fix the signature
    let signature = author.sign(&keystore, &action).await.unwrap();
    let op = DhtOp::RegisterAgentActivity(signature, action);
    let hash = DhtOpHash::with_data_sync(&op);

    // Run the workflow again to simulate a re-send of the op...
    incoming_dht_ops_workflow(
        space.space.clone(),
        sys_validation_trigger,
        vec![(hash.clone(), op)],
        true,
    )
    .await
    .unwrap();

    // ... and now it should succeed
    verify_ops_present(env, vec![hash], true).await;
    // then sys validation should run on the new op
    sys_validation_rx.listen().await.unwrap();
    // and no ops should be claimed for processing
    assert!(space.space.incoming_op_hashes.0.lock().is_empty());
}

async fn verify_ops_present(env: DbWrite<DbKindDht>, op_hashes: Vec<DhtOpHash>, present: bool) {
    env.read_async(move |txn| -> DatabaseResult<()> {
        for hash in op_hashes {
            let found: bool = txn
                .query_row(
                    "
                SELECT EXISTS(
                    SELECT 1 FROM DhtOP
                    WHERE when_integrated IS NULL
                    AND hash = :hash
                )
                ",
                    named_params! {
                        ":hash": hash,
                    },
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(present, found);
        }

        Ok(())
    })
    .await
    .unwrap();
}
