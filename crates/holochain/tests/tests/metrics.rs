use hdk::prelude::Record;
use holo_hash::ActionHash;
use holochain::sweettest::{SweetConductorBatch, SweetConductorConfig, SweetDnaFile};
use holochain_metrics::HolochainMetricsConfig;
use holochain_wasm_test_utils::TestWasm;
use serde::Serialize;
use std::fs::read_to_string;
use std::time::{Duration, Instant};

// Metrics checked for in this test:
// - hc.db.connections.use_time
// - hc.db.write_txn.duration
// - hc.conductor.workflow.duration
// - hc.conductor.workflow.integrated_ops
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
    HolochainMetricsConfig::new_with_file(&influxive_file, Some(Duration::from_secs(1)))
        .init()
        .await;

    #[derive(Debug, Serialize)]
    struct Post(pub String);

    // One wasm for creating entries and one for signaling.
    let (dna_file, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create, TestWasm::EmitSignal]).await;
    // Disable gossip and publish to make the network calls deterministic.
    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        2,
        SweetConductorConfig::standard().tune_network_config(|nc| {
            nc.disable_gossip = true;
            nc.disable_publish = true;
        }),
    )
    .await;

    let apps = conductors.setup_app("test_app", [&dna_file]).await.unwrap();
    let alice_conductor = conductors.get(0).unwrap();
    let bob_conductor = conductors.get(1).unwrap();
    let cells = apps.cells_flattened();
    let alice_cell = cells.first().unwrap();
    let alice_create_entry_zome = alice_cell.zome(TestWasm::Create.coordinator_zome());
    let alice_emit_signal_zome = alice_cell.zome(TestWasm::EmitSignal.coordinator_zome());
    let bob_cell = cells.get(1).unwrap();
    let bob_create_entry_zome = bob_cell.zome(TestWasm::Create.coordinator_zome());

    // Alice needs to answer Bob's get request, so she declares full storage arcs.
    alice_conductor
        .declare_full_storage_arcs(dna_file.dna_hash())
        .await;
    conductors.exchange_peer_info().await;

    let start = Instant::now();

    // Alice creates an entry to record zome call metrics.
    let create_entry_hash: ActionHash = alice_conductor
        .call(
            &alice_create_entry_zome,
            "create_post",
            Post("test".to_string()),
        )
        .await;
    // Bob gets Alice's entry to record network request metrics.
    let _: Option<Record> = bob_conductor
        .call(
            &bob_create_entry_zome,
            "get_post_network",
            create_entry_hash.clone(),
        )
        .await;
    // Alice emits a signal. For that to record a metric, a subscriber needs to be connected.
    let _signal_rx = alice_conductor.subscribe_to_app_signals("test_app".to_string());
    let _: () = alice_conductor
        .call(&alice_emit_signal_zome, "emit", ())
        .await;

    // Wait for metrics to be written.
    // Wait at least for 1 export.
    tokio::time::sleep(Duration::from_secs(2)).await;
    // Then wait until certain metrics are exported.
    let metrics = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let metrics = read_to_string(&influxive_file).unwrap_or_default();
            if metrics
                .matches("hc.holochain_p2p.handle_request.duration")
                .count()
                >= 2
            {
                return metrics;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("timed out waiting for metrics export");
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
    // 1 record per second for 6 database kinds (conductor, dht, cache, peer_meta_store, authored * 2)
    // cache may appear later, so assert >= 5
    assert!(
        db_connections_use_time_count >= expected_records_per_metric * 5,
        "hc.db.connections.use_time: expected >= {}, got {db_connections_use_time_count}",
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
    // 1 record per second for up to 6 database kinds; not all kinds receive write txns
    // in every run, so assert >= 4 (dht, authored * 2, conductor guaranteed)
    assert!(
        db_write_txn_duration_count >= expected_records_per_metric * 4,
        "hc.db.write_txn.duration: expected >= {}, got {db_write_txn_duration_count}",
        expected_records_per_metric * 4
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
        "hc.conductor.workflow.duration: expected >= {}, got {conductor_workflow_duration_count}",
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

    let conductor_workflow_integrated_ops_metric = metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.workflow.integrated_ops"));
    let conductor_workflow_integrated_ops_metric_count =
        conductor_workflow_integrated_ops_metric.clone().count();
    // 1 time series per DNA hash, so 1 record per second
    assert!(
        conductor_workflow_integrated_ops_metric_count >= expected_records_per_metric,
        "hc.conductor.workflow.integrated_ops: expected >= {expected_records_per_metric}, got {conductor_workflow_integrated_ops_metric_count}",
    );
    conductor_workflow_integrated_ops_metric.for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("sum="));
    });

    let conductor_post_commit_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.post_commit.duration"));
    let conductor_post_commit_duration_count = conductor_post_commit_duration.clone().count();
    // 1 record per second
    assert!(
        conductor_post_commit_duration_count >= expected_records_per_metric,
        "hc.conductor.post_commit.duration: expected >= {expected_records_per_metric}, got {conductor_post_commit_duration_count}",
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
    // ~5-6 distinct (dna, zome, fn) time series from wasm init + zome calls; assert >= 4
    assert!(
        ribosome_wasm_usage_count >= expected_records_per_metric * 4,
        "hc.ribosome.wasm.usage: expected >= {}, got {ribosome_wasm_usage_count}",
        expected_records_per_metric * 4
    );
    ribosome_wasm_usage.for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("zome="));
        assert!(metric.contains("fn="));
        assert!(metric.contains("sum="));
    });

    let ribosome_zome_call_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.zome_call.duration"));
    let ribosome_zome_call_duration_count = ribosome_zome_call_duration.clone().count();
    // At least 1 time series (create_post) exported every second.
    // get_post_network may happen late (after wasm compilation), so its time series may
    // have fewer exports — don't require both for the full duration.
    assert!(
        ribosome_zome_call_duration_count >= expected_records_per_metric,
        "hc.ribosome.zome_call.duration: expected >= {expected_records_per_metric}, got {ribosome_zome_call_duration_count}",
    );
    ribosome_zome_call_duration.for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("zome=create_entry") || metric.contains("zome=emit_signal"));
        assert!(metric.contains("fn="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    let mut ribosome_wasm_call_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.wasm_call.duration"));
    let ribosome_wasm_call_duration_count = ribosome_wasm_call_duration.clone().count();
    // ~5-6 distinct time series from all wasm sub-calls; assert >= 4
    assert!(
        ribosome_wasm_call_duration_count >= expected_records_per_metric * 4,
        "hc.ribosome.wasm_call.duration: expected >= {}, got {ribosome_wasm_call_duration_count}",
        expected_records_per_metric * 4
    );
    ribosome_wasm_call_duration.clone().for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("zome="));
        assert!(metric.contains("fn="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });
    // Check that some of the wasm calls have the zome and fn name of the
    // original zome call.
    assert!(ribosome_wasm_call_duration.any(
        |metric| metric.contains("zome=create_entry") && metric.contains("fn=get_post_network")
    ));

    let mut ribosome_host_fn_call_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.host_fn_call.duration"));
    let ribosome_host_fn_call_duration_count = ribosome_host_fn_call_duration.clone().count();
    // ~5-6 distinct (dna, zome, fn, host_fn) time series; assert >= 4
    assert!(
        ribosome_host_fn_call_duration_count >= expected_records_per_metric * 4,
        "hc.ribosome.host_fn_call.duration: expected >= {}, got {ribosome_host_fn_call_duration_count}",
        expected_records_per_metric * 4
    );
    ribosome_host_fn_call_duration.clone().for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("zome="));
        assert!(metric.contains("fn="));
        assert!(metric.contains("host_fn="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });
    // Check that some of the host fn calls have the zome and fn name of the
    // original zome call.
    assert!(ribosome_host_fn_call_duration.any(
        |metric| metric.contains("zome=create_entry") && metric.contains("fn=get_post_network")
    ));

    let ribosome_emit_signal_count = metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.host_fn.emit_signal.count"));
    let ribosome_emit_signal_count_count = ribosome_emit_signal_count.clone().count();
    // 1 signal emitted
    assert!(
        ribosome_emit_signal_count_count >= 1,
        "hc.ribosome.host_fn.emit_signal.count: expected >= 1, got {ribosome_emit_signal_count_count}",
    );
    // In influx line protocol, tag values escape commas and spaces with backslashes.
    let cell_id_influx = alice_cell
        .cell_id()
        .to_string()
        .replace(',', "\\,")
        .replace(' ', "\\ ");
    ribosome_emit_signal_count.clone().for_each(|metric| {
        assert!(metric.contains(&format!("cell_id={cell_id_influx}")));
        assert!(metric.contains("zome=emit_signal"));
        assert!(metric.contains("sum="));
    });

    // cascade metrics
    let cascade_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.cascade.duration"));
    let cascade_duration_count = cascade_duration.clone().count();
    // The cascade metric is first recorded during Bob's get_post_network call, which may happen
    // well after test start (wasm compilation etc.), so only assert >= 1 export occurred.
    assert!(
        cascade_duration_count >= 1,
        "hc.cascade.duration: expected >= 1, got {cascade_duration_count}",
    );
    cascade_duration.for_each(|metric| {
        // All cascade calls should have been made by the zome calls.
        assert!(metric.contains("zome=create_entry"));
        assert!(metric.contains("fn=get_post_network"));
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
    // Same timing caveat as cascade: first recorded during the network call, so assert >= 1.
    assert!(
        request_duration_count >= 1,
        "hc.holochain_p2p.request.duration: expected >= 1, got {request_duration_count}",
    );
    request_duration.for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("tag="));
        assert!(metric.contains("url="));
        assert!(metric.contains("error="));
        // All network requests should have been made by the zome calls.
        assert!(metric.contains("zome=create_entry"));
        assert!(metric.contains("fn=get_post_network"));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    let handle_request_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.holochain_p2p.handle_request.duration"));
    let handle_request_duration_count = handle_request_duration.clone().count();
    // 2-3 distinct time series (GetReq, GetRes, plus validation receipt traffic).
    // Like cascade and request above, these are first recorded when the network call happens,
    // so the effective export window is shorter than seconds_elapsed; assert >= 1.
    assert!(
        handle_request_duration_count >= 1,
        "hc.holochain_p2p.handle_request.duration: expected >= 1, got {handle_request_duration_count}",
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
