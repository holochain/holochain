use hdk::prelude::Record;
use holo_hash::ActionHash;
use holochain::sweettest::{await_consistency, SweetConductorBatch, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;
use serde::Serialize;
use std::fs::read_to_string;
use std::time::{Duration, Instant};

// Metrics checked for in this test:
// - hc.db.connections.use_time
// - hc.db.write_txn.duration
// - hc.conductor.workflow.duration
// - hc.conductor.post_commit.duration
// - hc.ribosome.wasm.usage
// - hc.ribosome.zome_call.duration
// - hc.ribosome.wasm_call.duration
// - hc.ribosome.host_fn_call.duration
// - hc.cascade.duration
// - hc.holochain_p2p.request.duration
// - hc.holochain_p2p.handle_request.duration
#[tokio::test(flavor = "multi_thread")]
async fn metrics() {
    let tmp_file = tempfile::tempdir().unwrap();
    let influxive_file = tmp_file.path().join("metrics.influx");
    holochain_metrics::HolochainMetricsConfig::with_file(
        &influxive_file,
        Some(Duration::from_secs(1)),
    )
    .init()
    .await;

    #[derive(Debug, Serialize)]
    struct Post(pub String);

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductors = SweetConductorBatch::standard(2).await;

    let apps = conductors.setup_app("test_app", [&dna_file]).await.unwrap();
    let alice_conductor = conductors.get(0).unwrap();
    let bob_conductor = conductors.get(1).unwrap();
    let cells = apps.cells_flattened();
    let alice_cell = cells.get(0).unwrap();
    let alice_zome = alice_cell.zome(TestWasm::Create.coordinator_zome());
    let bob_cell = cells.get(1).unwrap();
    let bob_zome = bob_cell.zome(TestWasm::Create.coordinator_zome());

    let start = Instant::now();
    let mut get_requests = 0;

    // Alice creates an entry to record zome call metrics.
    let create_entry_hash: ActionHash = alice_conductor
        .call(&alice_zome, "create_post", Post("test".to_string()))
        .await;
    // Bob gets Alice's entry to record network request metrics.
    let _: Option<Record> = bob_conductor
        .call(&bob_zome, "get_post_network", create_entry_hash.clone())
        .await;
    get_requests += 1;

    await_consistency(&apps.cells_flattened()).await.unwrap();

    // Wait for buffered metrics to be flushed.
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Read metrics from file
    let metrics = read_to_string(influxive_file).unwrap();
    println!("{metrics}");
    let metrics = metrics.lines();

    let seconds_elapsed = start.elapsed().as_secs() as usize;
    // It could be that the last record didn't get exported, so seconds_elapsed - 1.
    let expected_records_per_metric = seconds_elapsed - 1;
    println!("{seconds_elapsed} s elapsed");
    println!("Expected {expected_records_per_metric} records per metric");

    // METRIC ASSERTIONS

    // db metrics
    let db_connections_use_time = metrics
        .clone()
        .filter(|line| line.contains("hc.db.connections.use_time"));
    let db_connections_use_time_count = db_connections_use_time.clone().count();
    // 1 record per second for 5 database kinds (dht, cache, peer_meta_store, authored * 2)
    assert!(
        db_connections_use_time_count >= expected_records_per_metric * 5,
        "expected >= {}, got {db_connections_use_time_count}",
        expected_records_per_metric * 5
    );
    db_connections_use_time.for_each(|metric| {
        assert!(metric.contains("id="));
        assert!(metric.contains("kind="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    let db_write_txn_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.db.write_txn.duration"));
    let db_write_txn_duration_count = db_write_txn_duration.clone().count();
    // 1 record per second for 5 database kinds (dht, cache, peer_meta_store, authored * 2)
    assert!(
        db_write_txn_duration_count >= expected_records_per_metric * 5,
        "expected >= {}, got {db_write_txn_duration_count}",
        expected_records_per_metric * 5
    );
    db_write_txn_duration.for_each(|metric| {
        assert!(metric.contains("id="));
        assert!(metric.contains("kind="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    // conductor metrics
    let conductor_workflow_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.workflow.duration"));
    let conductor_workflow_duration_count = conductor_workflow_duration.clone().count();
    // 1 record per second for 8 workflows (publish_dht_ops_consumer * 2, countersigning_consumer * 2
    // app_validation_consumer, sys_validation_consumer, integrate_dht_ops_consumer, validation_receipt_consumer)
    assert!(
        conductor_workflow_duration_count >= expected_records_per_metric * 8,
        "expected >= {}, got {conductor_workflow_duration_count}",
        expected_records_per_metric * 8
    );
    conductor_workflow_duration.for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("workflow="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    let conductor_post_commit_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.post_commit.duration"));
    let conductor_post_commit_duration_count = conductor_post_commit_duration.clone().count();
    // 1 record per second for 2 agents having committed
    assert!(
        conductor_post_commit_duration_count >= expected_records_per_metric * 2,
        "expected >= {}, got {conductor_post_commit_duration_count}",
        expected_records_per_metric * 2,
    );
    conductor_post_commit_duration.for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("agent="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    // Ribosome metrics
    let ribosome_wasm_usage = metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.wasm.usage"));
    let ribosome_wasm_usage_count = ribosome_wasm_usage.clone().count();
    // 10 records per second
    assert!(
        ribosome_wasm_usage_count >= expected_records_per_metric * 10,
        "expected >= {}, got {ribosome_wasm_usage_count}",
        expected_records_per_metric * 10
    );
    ribosome_wasm_usage.for_each(|metric| {
        assert!(metric.contains("dna="));
        assert!(metric.contains("zome="));
        assert!(metric.contains("fn="));
        assert!(metric.contains("sum="));
    });

    let ribosome_zome_call_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.zome_call.duration"));
    let ribosome_zome_call_duration_count = ribosome_zome_call_duration.clone().count();
    // 2 records per second (create_post, get_post_network)
    assert!(
        ribosome_zome_call_duration_count >= expected_records_per_metric * 2,
        "expected >= {}, got {ribosome_zome_call_duration_count}",
        expected_records_per_metric * 2
    );
    ribosome_zome_call_duration.for_each(|metric| {
        assert!(metric.contains("dna="));
        assert!(metric.contains("zome=create_entry"));
        assert!(metric.contains("fn="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    let ribosome_wasm_call_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.wasm_call.duration"));
    let ribosome_wasm_call_duration_count = ribosome_wasm_call_duration.clone().count();
    // 10 records per second
    assert!(
        ribosome_wasm_call_duration_count >= expected_records_per_metric * 10,
        "expected >= {}, got {ribosome_wasm_call_duration_count}",
        expected_records_per_metric * 10
    );
    ribosome_wasm_call_duration.for_each(|metric| {
        assert!(metric.contains("dna="));
        assert!(metric.contains("zome=create_entry"));
        assert!(metric.contains("fn="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    let ribosome_host_fn_call_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.host_fn_call.duration"));
    let ribosome_host_fn_call_duration_count = ribosome_host_fn_call_duration.clone().count();
    // 7 records per second
    assert!(
        ribosome_host_fn_call_duration_count >= expected_records_per_metric * 7,
        "expected >= {}, got {ribosome_host_fn_call_duration_count}",
        expected_records_per_metric * 7
    );
    ribosome_host_fn_call_duration.for_each(|metric| {
        assert!(metric.contains("dna="));
        assert!(metric.contains("zome=create_entry"));
        assert!(metric.contains("fn="));
        assert!(metric.contains("host_fn="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    // cascade metrics
    let cascade_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.cascade.duration"));
    let cascade_duration_count = cascade_duration.clone().count();
    // 1 record per get request
    assert!(
        cascade_duration_count >= get_requests,
        "expected >= {get_requests}, got {cascade_duration_count}",
    );
    cascade_duration.for_each(|metric| {
        // All cascade calls should have been made by the zome calls.
        assert!(metric.contains("zome_name="));
        assert!(metric.contains("fn_name="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    // holochain_p2p metrics
    let request_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.holochain_p2p.request.duration"));
    let request_duration_count = request_duration.clone().count();
    // 1 record per get request
    assert!(
        request_duration_count >= get_requests,
        "expected >= {get_requests}, got {request_duration_count}",
    );
    request_duration.for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("tag="));
        assert!(metric.contains("url="));
        assert!(metric.contains("error="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    let handle_request_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.holochain_p2p.handle_request.duration"));
    let handle_request_duration_count = handle_request_duration.clone().count();
    // 1 record per get request
    assert!(
        handle_request_duration_count >= get_requests,
        "expected >= {get_requests}, got {handle_request_duration_count}",
    );
    handle_request_duration.for_each(|metric| {
        assert!(metric.contains("message_type="));
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    // hc.holochain_p2p.handle_request.ignored can't be easily tested, because
    // it records a metric only when concurrent requests are handled and one
    // of them is dropped.
}
