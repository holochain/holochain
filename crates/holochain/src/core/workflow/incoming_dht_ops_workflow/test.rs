use super::*;
use ::fixt::prelude::*;
use holochain_keystore::AgentPubKeyExt;

#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_to_limbo() {
    let test_env = holochain_state::test_utils::test_cell_env();
    let env = test_env.env();
    let keystore = holochain_state::test_utils::test_keystore();
    let (sys_validation_trigger, mut rx) = TriggerSender::new();

    let author = fake_agent_pubkey_1();
    let mut header = fixt!(CreateLink);
    header.author = author.clone();
    let header = Header::CreateLink(header);
    let signature = author.sign(&keystore, &header).await.unwrap();

    let op = DhtOp::RegisterAgentActivity(signature, header);
    let hash = DhtOpHash::with_data_sync(&op);
    let ops = vec![(hash.clone(), op.clone())];

    incoming_dht_ops_workflow(&env, sys_validation_trigger.clone(), ops, None, false)
        .await
        .unwrap();
    rx.listen().await.unwrap();

    fresh_reader_test(env, |txn| {
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
    });
}
