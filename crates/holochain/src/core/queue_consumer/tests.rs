use super::*;

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
    let (t, mut result) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move {
        loop {
            rx.listen().await.unwrap();
            t.send(()).await.unwrap();
        }
    });
    // After 10ms there should be no trigger.
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    assert!(r.is_err());

    // After 60s there should be one trigger.
    tokio::time::sleep(Duration::from_secs(60)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // After another 70s there should be no trigger.
    let r = tokio::time::timeout(Duration::from_secs(70), result.recv()).await;
    assert!(r.is_err());

    // After 50s (total of 120s since last success) there should be one trigger.
    tokio::time::sleep(Duration::from_secs(50)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // After another 130s there should be no trigger.
    let r = tokio::time::timeout(Duration::from_secs(130), result.recv()).await;
    assert!(r.is_err());

    // After 110s (total of 240s since last success) there should be one trigger.
    tokio::time::sleep(Duration::from_secs(110)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // After another 250s there should be no trigger.
    let r = tokio::time::timeout(Duration::from_secs(250), result.recv()).await;
    assert!(r.is_err());

    // After 50s (total of 300ms since last success) there should be one trigger.
    tokio::time::sleep(Duration::from_secs(50)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // After another 290s there should be no trigger.
    let r = tokio::time::timeout(Duration::from_secs(290), result.recv()).await;
    assert!(r.is_err());

    // After 10s (total of 300ms since last success) there should be one trigger.
    tokio::time::sleep(Duration::from_secs(10)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    tx.reset_back_off();

    // After 300s (total of 300ms since last success) there should be one trigger.
    tokio::time::sleep(Duration::from_secs(300)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // Now the back off is reset.

    // After 10ms there should be no trigger.
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    assert!(r.is_err());

    // After 60s there should be one trigger.
    tokio::time::sleep(Duration::from_secs(60)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_trigger_loop() {
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60), false);
    let (t, mut result) = tokio::sync::mpsc::channel(100);

    tokio::spawn(async move {
        loop {
            rx.listen().await.unwrap();
            t.send(()).await.unwrap();
        }
    });

    // After 10ms there should be no trigger.
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    assert!(r.is_err());

    // After 60s there should be one trigger.
    tokio::time::sleep(Duration::from_secs(60)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // After 10ms there should be no trigger.
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    assert!(r.is_err());

    // After 60s there should be one trigger.
    tokio::time::sleep(Duration::from_secs(60)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    tx.trigger();
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // After 10ms there should be no trigger.
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    assert!(r.is_err());

    // After 60s there should be one trigger.
    tokio::time::sleep(Duration::from_secs(60)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_reset_on_trigger() {
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60 * 5), true);
    let (t, mut result) = tokio::sync::mpsc::channel(100);

    tokio::spawn(async move {
        loop {
            rx.listen().await.unwrap();
            t.send(()).await.unwrap();
        }
    });

    // After 10ms there should be no trigger.
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    assert!(r.is_err());

    // After 60s there should be one trigger.
    tokio::time::sleep(Duration::from_secs(60)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // After another 70s there should be no trigger.
    let r = tokio::time::timeout(Duration::from_secs(70), result.recv()).await;
    assert!(r.is_err());

    // After 50s (total of 120s since last success) there should be one trigger.
    tokio::time::sleep(Duration::from_secs(50)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    tx.trigger();
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // No immediate trigger after the reset.
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    assert!(r.is_err());

    // However after 60s (because the back off is reset) there should be one trigger.
    tokio::time::sleep(Duration::from_secs(60)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_pause_resume() {
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60), false);
    let (t, mut result) = tokio::sync::mpsc::channel(100);

    tokio::spawn(async move {
        loop {
            rx.listen().await.unwrap();
            t.send(()).await.unwrap();
        }
    });

    for _ in 0..10 {
        // After 10ms there should be no trigger.
        let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
        assert!(r.is_err());

        // After 60s there should be one trigger.
        tokio::time::sleep(Duration::from_secs(60)).await;
        let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
        r.unwrap().unwrap();
    }

    tx.pause_loop();
    // After 1hr there should be no trigger.
    let r = tokio::time::timeout(Duration::from_secs(60 * 60), result.recv()).await;
    assert!(r.is_err());

    tx.resume_loop();
    // Loop should continue.
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();

    // After 10ms there should be no trigger.
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    assert!(r.is_err());

    // After 60s there should be one trigger.
    tokio::time::sleep(Duration::from_secs(60)).await;
    let r = tokio::time::timeout(Duration::from_millis(10), result.recv()).await;
    r.unwrap().unwrap();
}
