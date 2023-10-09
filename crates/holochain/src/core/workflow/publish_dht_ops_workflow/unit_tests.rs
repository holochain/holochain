use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use crate::prelude::AgentPubKeyFixturator;
use crate::prelude::MockHolochainP2pDnaT;
use fixt::prelude::*;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn no_ops_to_publish() {
    holochain_trace::test_run().ok();

    let test_db = holochain_state::test_utils::test_authored_db();
    let vault = test_db.to_db();
    let keystore = holochain_state::test_utils::test_keystore();

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_publish().never(); // Verify no receipts sent

    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(5)..Duration::from_secs(30), true);

    let work_complete = publish_dht_ops_workflow(vault, Arc::new(network), tx, fixt!(AgentPubKey))
        .await
        .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);
}
