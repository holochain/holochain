use super::*;
use ::fixt::prelude::*;
use holochain_keystore::AgentPubKeyExt;

#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_to_limbo() {
    let test_env = holochain_lmdb::test_utils::test_cell_env();
    let env = test_env.env();
    let keystore = holochain_lmdb::test_utils::test_keystore();
    let (sys_validation_trigger, mut rx) = TriggerSender::new();

    let author = fake_agent_pubkey_1();
    let mut header = fixt!(CreateLink);
    header.author = author.clone();
    let header = Header::CreateLink(header);
    let signature = author.sign(&keystore, &header).await.unwrap();

    let op = DhtOp::RegisterAgentActivity(signature, header);
    let op_light = op.to_light();
    let hash = DhtOpHash::with_data_sync(&op);
    let ops = vec![(hash.clone(), op.clone())];

    incoming_dht_ops_workflow(&env, sys_validation_trigger.clone(), ops, None)
        .await
        .unwrap();
    rx.listen().await.unwrap();

    let workspace = IncomingDhtOpsWorkspace::new(env.clone().into()).unwrap();
    let r = workspace.validation_limbo.get(&hash).unwrap().unwrap();
    assert_eq!(r.op, op_light);
}
