use super::*;
use ::fixt::prelude::*;
use holochain_keystore::AgentPubKeyExt;

#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_to_limbo() {
    let test_env = holochain_state::test_utils::test_cell_env();
    let env = test_env.env();
    let keystore = holochain_state::test_utils::test_keystore();

    let author = fake_agent_pubkey_1();

    let mut hash_list = Vec::new();
    let mut op_list = Vec::new();

    for _ in 0..10 {
        let mut header = fixt!(CreateLink);
        header.author = author.clone();
        let header = Header::CreateLink(header);
        let signature = author.sign(&keystore, &header).await.unwrap();

        let op = DhtOp::RegisterAgentActivity(signature, header);
        let hash = DhtOpHash::with_data_sync(&op);
        hash_list.push(hash.clone());
        op_list.push((hash, op));
    }

    let mut all = Vec::new();
    for op in op_list {
        let (sys_validation_trigger, _) = TriggerSender::new();
        let env = env.clone();
        all.push(tokio::task::spawn(async move {
            let start = std::time::Instant::now();
            incoming_dht_ops_workflow(&env, sys_validation_trigger, vec![op], None, false)
                .await
                .unwrap();
            println!("IN OP in {} s", start.elapsed().as_secs_f64());
        }));
    }

    futures::future::try_join_all(all).await.unwrap();

    fresh_reader_test(env, |txn| {
        for hash in hash_list {
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
            assert!(found);
        }
    });
}
