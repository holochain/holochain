use super::*;
use fixt::prelude::*;
use holochain_state::test_utils::TestEnvironment;
use holochain_types::{dht_op::DhtOp, fixt::*};

#[tokio::test(threaded_scheduler)]
async fn incoming_ops_to_limbo() {
    let TestEnvironment { env, tmpdir: _t } = holochain_state::test_utils::test_cell_env();
    let (sys_validation_trigger, mut rx) = TriggerSender::new();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), fixt!(Header));
    let hash = DhtOpHash::with_data(&op).await;
    let ops = vec![(hash.clone(), op.clone())];

    incoming_dht_ops_workflow(&env, sys_validation_trigger.clone(), ops)
        .await
        .unwrap();
    rx.listen().await.unwrap();

    let env_ref = env.guard().await;
    let reader = env_ref.reader().unwrap();
    let workspace = IncomingDhtOpsWorkspace::new(env.clone().into(), &env_ref).unwrap();
    let r = workspace
        .validation_limbo
        .get(&hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(r.op, op);
}
