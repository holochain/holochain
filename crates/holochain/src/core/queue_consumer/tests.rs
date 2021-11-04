use crate::core::workflow::publish_dht_ops_workflow::{
    publish_dht_ops_workflow, MIN_PUBLISH_INTERVAL,
};

use super::*;
use arbitrary::Arbitrary;
use holochain_sqlite::db::{DbKind, WriteManager};
use holochain_state::{mutations, prelude::mutations_helpers};

#[tokio::test]
async fn test_trigger() {
    let (_tx, mut rx) = TriggerSender::new();
    let jh = tokio::spawn(async move { rx.listen().await.unwrap() });

    // This should timeout because the trigger was not called.
    let r = tokio::time::timeout(Duration::from_millis(10), jh).await;
    assert!(r.is_err());

    let (tx, mut rx) = TriggerSender::new();
    let jh = tokio::spawn(async move { rx.listen().await.unwrap() });
    tx.trigger();

    // This should be joined because the trigger was called.
    let r = jh.await;
    assert!(r.is_ok());

    let (tx, mut rx) = TriggerSender::new();
    let jh = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        rx.listen().await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        rx.listen().await.unwrap()
    });
    // Calling trigger twice before a listen should only
    // cause one listen to progress.
    tx.trigger();
    tx.trigger();

    // This should timeout because the second listen should not pass.
    let r = tokio::time::timeout(Duration::from_millis(100), jh).await;
    assert!(r.is_err());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_trigger_back_off() {
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60 * 5), false);

    // After 1m there should be one trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
    );

    // After 2m there should be one trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60 * 2)
            && timer.elapsed() < Duration::from_secs(60 * 2 + 1)
    );

    // After 4m there should be one trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60 * 4)
            && timer.elapsed() < Duration::from_secs(60 * 4 + 1)
    );

    // After 5m there should be one trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60 * 5)
            && timer.elapsed() < Duration::from_secs(60 * 5 + 1)
    );

    // After 5m there should be another trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60 * 5)
            && timer.elapsed() < Duration::from_secs(60 * 5 + 1)
    );

    tx.reset_back_off();

    // After 1m there should be one trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_trigger_loop() {
    let (_tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60), false);

    for _ in 0..100 {
        // After 1m there should be one trigger.
        let timer = tokio::time::Instant::now();
        rx.listen().await.unwrap();
        assert!(
            timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
        );
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_reset_on_trigger() {
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60 * 5), true);

    // After 1m there should be one trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
    );

    // After 2m there should be one trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60 * 2)
            && timer.elapsed() < Duration::from_secs(60 * 2 + 1)
    );

    tx.trigger();

    // There should be one trigger immediately.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(timer.elapsed() < Duration::from_secs(1));

    // After 1m there should be one trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_pause_resume() {
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60), false);

    for _ in 0..10 {
        // After 1m there should be one trigger.
        let timer = tokio::time::Instant::now();
        rx.listen().await.unwrap();
        assert!(
            timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
        );
    }

    tx.pause_loop();

    // After 1hr there should be no trigger.
    let r = tokio::time::timeout(Duration::from_secs(60 * 60), rx.listen()).await;
    assert!(r.is_err());

    tx.resume_loop();

    // After 1m there should be one trigger.
    let timer = tokio::time::Instant::now();
    rx.listen().await.unwrap();
    assert!(
        timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
    );
}

#[tokio::test]
async fn test_concurrency() {
    // - Trigger overrides already waiting listen.
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_millis(60)..Duration::from_millis(60), false);
    let timer = tokio::time::Instant::now();
    let jh = tokio::spawn(async move { rx.listen().await.unwrap() });
    // - Make sure listen has been called already.
    tokio::time::sleep(Duration::from_millis(10)).await;
    tx.trigger();
    jh.await.unwrap();
    assert!(timer.elapsed() < Duration::from_millis(20));

    // - Calling resume_loop_now doesn't override waiting listen when loop is running.
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_millis(60)..Duration::from_millis(60), false);
    let timer = tokio::time::Instant::now();
    let jh = tokio::spawn(async move { rx.listen().await.unwrap() });
    // - Make sure listen has been called already.
    tokio::time::sleep(Duration::from_millis(10)).await;
    tx.resume_loop_now();
    jh.await.unwrap();
    assert!(timer.elapsed() >= Duration::from_millis(60));

    // - Calling resume_loop_now does override waiting listen when loop is paused.
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_millis(60)..Duration::from_millis(60), false);
    tx.pause_loop();
    let timer = tokio::time::Instant::now();
    let jh = tokio::spawn(async move { rx.listen().await.unwrap() });
    // - Make sure listen has been called already.
    tokio::time::sleep(Duration::from_millis(10)).await;
    tx.resume_loop_now();
    jh.await.unwrap();
    assert!(timer.elapsed() < Duration::from_millis(20));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn publish_loop() {
    let mut u = arbitrary::Unstructured::new(&[0; 1000]);
    // HNNRGG... new lair keystore is multi_thread...
    //           it won't work for this test...
    //           we may have to make a dummy mock keystore here
    let keystore = holochain_keystore::test_keystore::spawn_legacy_test_keystore()
        .await
        .unwrap();
    let kind = DbKind::Cell(CellId::new(
        DnaHash::arbitrary(&mut u).unwrap(),
        AgentPubKey::arbitrary(&mut u).unwrap(),
    ));
    let tmpdir = tempdir::TempDir::new("holochain-test-environments").unwrap();
    let env = EnvWrite::test(&tmpdir, kind, keystore).expect("Couldn't create test database");
    let header = Header::arbitrary(&mut u).unwrap();
    let author = header.author().clone();
    let signature = Signature::arbitrary(&mut u).unwrap();
    let op = DhtOp::RegisterAgentActivity(signature, header);
    let op = DhtOpHashed::from_content_sync(op);
    let op_hash = op.to_hash();
    env.conn()
        .unwrap()
        .with_commit_test(|txn| {
            mutations_helpers::insert_valid_authored_op(txn, op).unwrap();
        })
        .unwrap();
    let mut dna_network = MockHolochainP2pDnaT::new();
    let (tx, mut op_published) = tokio::sync::mpsc::channel(100);
    dna_network
        .expect_publish()
        .returning(move |_, _, _, _, _| {
            tx.try_send(()).unwrap();
            Ok(())
        });

    let (ts, mut trigger_recv) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60 * 5), true);

    let timer = tokio::time::Instant::now();
    trigger_recv.listen().await.unwrap();
    // - Publish runs after a 1m.
    assert!(
        timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
    );

    publish_dht_ops_workflow(env.clone(), &dna_network, &ts, author.clone())
        .await
        .unwrap();

    // - Op was published.
    op_published.recv().await.unwrap();

    let timer = tokio::time::Instant::now();
    trigger_recv.listen().await.unwrap();
    // - Publish runs again after 2m.
    assert!(
        timer.elapsed() >= Duration::from_secs(60 * 2)
            && timer.elapsed() < Duration::from_secs(60 * 2 + 1)
    );
    publish_dht_ops_workflow(env.clone(), &dna_network, &ts, author.clone())
        .await
        .unwrap();

    // - But the op isn't published because it was published in the last five minutes.
    assert_eq!(
        op_published.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );

    // - Triggering publish causes it to run again.
    ts.trigger();

    let timer = tokio::time::Instant::now();
    trigger_recv.listen().await.unwrap();
    assert!(timer.elapsed() < Duration::from_secs(1));
    publish_dht_ops_workflow(env.clone(), &dna_network, &ts, author.clone())
        .await
        .unwrap();

    // - But still no op is published.
    assert_eq!(
        op_published.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );

    // - Set the ops last publish time to five mins ago.
    let five_mins_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|epoch| epoch.checked_sub(MIN_PUBLISH_INTERVAL))
        .unwrap();

    env.conn()
        .unwrap()
        .with_commit_test(|txn| {
            mutations::set_last_publish_time(txn, op_hash.clone(), five_mins_ago).unwrap();
        })
        .unwrap();

    let timer = tokio::time::Instant::now();
    trigger_recv.listen().await.unwrap();
    // - Publish runs after a 1m because the trigger reset the back off.
    assert!(
        timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
    );

    publish_dht_ops_workflow(env.clone(), &dna_network, &ts, author.clone())
        .await
        .unwrap();

    // - The data is published because of the last publish time being greater then the interval.
    op_published.recv().await.unwrap();

    // - Set receipts complete.
    env.conn()
        .unwrap()
        .with_commit_test(|txn| {
            mutations::set_receipts_complete(txn, &op_hash, true).unwrap();
        })
        .unwrap();

    let timer = tokio::time::Instant::now();
    trigger_recv.listen().await.unwrap();
    // - Publish runs after another 2ms.
    assert!(
        timer.elapsed() >= Duration::from_secs(60 * 2)
            && timer.elapsed() < Duration::from_secs(60 * 2 + 1)
    );
    publish_dht_ops_workflow(env.clone(), &dna_network, &ts, author.clone())
        .await
        .unwrap();

    // - But no op is published because receipts are complete.
    assert_eq!(
        op_published.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );

    // - Confirm the publish loop doesn't run again.
    let r = tokio::time::timeout(Duration::from_secs(60 * 100), trigger_recv.listen()).await;
    assert!(r.is_err());
    assert_eq!(
        op_published.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );

    // - Set the ops last publish time to five mins ago.
    // - Set receipts not complete.
    env.conn()
        .unwrap()
        .with_commit_test(|txn| {
            mutations::set_last_publish_time(txn, op_hash.clone(), five_mins_ago).unwrap();
            mutations::set_receipts_complete(txn, &op_hash, false).unwrap();
        })
        .unwrap();

    // - Publish runs due to a trigger.
    ts.trigger();
    let timer = tokio::time::Instant::now();
    trigger_recv.listen().await.unwrap();
    assert!(timer.elapsed() < Duration::from_secs(1));
    publish_dht_ops_workflow(env.clone(), &dna_network, &ts, author.clone())
        .await
        .unwrap();

    // - Op was published.
    op_published.recv().await.unwrap();

    let timer = tokio::time::Instant::now();
    trigger_recv.listen().await.unwrap();
    // - Publish is looping again starting at 1m.
    assert!(
        timer.elapsed() >= Duration::from_secs(60) && timer.elapsed() < Duration::from_secs(61)
    );

    publish_dht_ops_workflow(env.clone(), &dna_network, &ts, author.clone())
        .await
        .unwrap();
    // - The op is not published because of the time interval.
    assert_eq!(
        op_published.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
}
