use super::*;
use fixt::prelude::*;
use holochain_state::test_utils::TestEnvironment;
use holochain_types::{dht_op::DhtOp, fixt::*};

#[tokio::test(threaded_scheduler)]
async fn incoming_ops_to_limbo() {
    let TestEnvironment { env, tmpdir: _t } = holochain_state::test_utils::test_cell_env();
    let (sys_validation_trigger, mut rx) = TriggerSender::new();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), fixt!(Header));
    let op_light = op.to_light().await;
    let hash = DhtOpHash::with_data_sync(&op);
    let ops = vec![(hash.clone(), op.clone())];

    incoming_dht_ops_workflow(&env, sys_validation_trigger.clone(), ops)
        .await
        .unwrap();
    rx.listen().await.unwrap();

    let workspace = IncomingDhtOpsWorkspace::new(env.clone().into()).unwrap();
    let r = workspace.validation_limbo.get(&hash).unwrap().unwrap();
    assert_eq!(r.op, op_light);
}
