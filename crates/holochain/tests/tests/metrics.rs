use hdk::prelude::Record;
use holo_hash::ActionHash;
use holochain::prelude::ExternIO;
use holochain::sweettest::{SweetConductorBatch, SweetConductorConfig, SweetDnaFile};
use holochain_metrics::HolochainMetricsConfig;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::RemoteSignal;
use serde::Serialize;
use std::fs::read_to_string;
use std::time::Duration;

// Metrics checked for in this test:
// - hc.db.connections.use_time
// - hc.db.write_txn.duration
// - hc.conductor.workflow.duration
// - hc.conductor.workflow.integrated_ops
// - hc.conductor.workflow.integration_delay
// - hc.conductor.post_commit.duration
// - hc.conductor.uptime
// - hc.ribosome.wasm.usage
// - hc.ribosome.zome_call.duration
// - hc.ribosome.wasm_call.duration
// - hc.ribosome.host_fn_call.duration
// - hc.ribosome.host_fn.emit_signal
// - hc.ribosome.host_fn.send_remote_signal
// - hc.cascade.duration
// - hc.holochain_p2p.request.duration
// - hc.holochain_p2p.handle_request.duration
// - hc.holochain_p2p.recv_remote_signal
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
    // Alice sends a remote signal to Bob.
    let _: () = alice_conductor
        .call(
            &alice_emit_signal_zome,
            "signal_others",
            RemoteSignal {
                agents: vec![bob_cell.agent_pubkey().clone()],
                signal: ExternIO::encode(()).unwrap(),
            },
        )
        .await;

    // Wait until the influx file contains enough exported records to satisfy every assertion below.
    let metrics = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let metrics = read_to_string(&influxive_file).unwrap();
            if metrics.matches("hc.db.connections.use_time").count() >= 5
                && metrics.matches("hc.db.write_txn.duration").count() >= 4
                && metrics.matches("hc.conductor.workflow.duration").count() >= 8
                && metrics
                    .matches("hc.conductor.workflow.integrated_ops")
                    .count()
                    >= 1
                && metrics
                    .matches("hc.conductor.workflow.integration_delay")
                    .count()
                    >= 1
                && metrics.matches("hc.conductor.post_commit.duration").count() >= 1
                && metrics.matches("hc.conductor.uptime").count() >= 1
                && metrics.matches("hc.ribosome.wasm.usage").count() >= 4
                && metrics.matches("hc.ribosome.zome_call.duration").count() >= 1
                && metrics.matches("hc.ribosome.wasm_call.duration").count() >= 4
                && metrics.matches("hc.ribosome.host_fn_call.duration").count() >= 4
                && metrics.contains("hc.cascade.duration")
                && metrics.matches("hc.holochain_p2p.request.duration").count() >= 1
                && metrics.contains("hc.holochain_p2p.handle_request.duration")
                && metrics.contains("hc.holochain_p2p.recv_remote_signal")
                && metrics.matches("hc.ribosome.host_fn.emit_signal").count() >= 1
                && metrics
                    .matches("hc.ribosome.host_fn.send_remote_signal")
                    .count()
                    >= 1
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

    // METRIC ASSERTIONS

    // db metrics
    metrics
        .clone()
        .filter(|line| line.contains("hc.db.connections.use_time"))
        .for_each(|metric| {
            assert!(metric.contains("id="));
            assert!(metric.contains("kind="));
            assert!(metric.contains("count="));
            assert!(metric.contains("sum="));
            assert!(metric.contains("max="));
            assert!(metric.contains("min="));
        });

    metrics
        .clone()
        .filter(|line| line.contains("hc.db.write_txn.duration"))
        .for_each(|metric| {
            assert!(metric.contains("id="));
            assert!(metric.contains("kind="));
            assert!(metric.contains("count="));
            assert!(metric.contains("sum="));
            assert!(metric.contains("max="));
            assert!(metric.contains("min="));
        });

    // conductor metrics
    metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.workflow.duration"))
        .for_each(|metric| {
            assert!(metric.contains("dna_hash="));
            assert!(metric.contains("workflow="));
            assert!(metric.contains("count="));
            assert!(metric.contains("sum="));
            assert!(metric.contains("max="));
            assert!(metric.contains("min="));
        });

    metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.workflow.integrated_ops"))
        .for_each(|metric| {
            assert!(metric.contains("dna_hash="));
            assert!(metric.contains("sum="));
        });

    metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.workflow.integration_delay"))
        .for_each(|metric| {
            assert!(metric.contains("dna_hash="));
            assert!(metric.contains("count="));
            assert!(metric.contains("sum="));
            assert!(metric.contains("max="));
            assert!(metric.contains("min="));
        });

    metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.post_commit.duration"))
        .for_each(|metric| {
            assert!(metric.contains("dna_hash="));
            assert!(metric.contains("agent="));
            assert!(metric.contains("count="));
            assert!(metric.contains("sum="));
            assert!(metric.contains("max="));
            assert!(metric.contains("min="));
        });

    metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.uptime"))
        .for_each(|metric| {
            assert!(metric.contains("gauge="));
        });

    // Ribosome metrics
    metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.wasm.usage"))
        .for_each(|metric| {
            assert!(metric.contains("dna_hash="));
            assert!(metric.contains("zome="));
            assert!(metric.contains("fn="));
            assert!(metric.contains("sum="));
        });

    metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.zome_call.duration"))
        .for_each(|metric| {
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

    // Signals
    // In influx line protocol, tag values escape commas and spaces with backslashes.
    let cell_id_influx = alice_cell
        .cell_id()
        .to_string()
        .replace(',', "\\,")
        .replace(' ', "\\ ");
    metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.host_fn.emit_signal"))
        .for_each(|metric| {
            assert!(metric.contains(&format!("cell_id={cell_id_influx}")));
            assert!(metric.contains("zome=emit_signal"));
            // Assert that emit signal was recorded.
            assert!(metric.contains("sum=1u"));
        });

    metrics
        .clone()
        .filter(|line| line.contains("hc.ribosome.host_fn.send_remote_signal"))
        .for_each(|metric| {
            assert!(metric.contains("dna_hash="));
            assert!(metric.contains("zome=emit_signal"));
            // Assert that remote signal send was recorded.
            assert!(metric.contains("sum=1u"));
        });

    // cascade metrics
    metrics
        .clone()
        .filter(|line| line.contains("hc.cascade.duration"))
        .for_each(|metric| {
            // All cascade calls should have been made by the zome calls.
            assert!(metric.contains("zome=create_entry"));
            assert!(metric.contains("fn=get_post_network"));
            assert!(metric.contains("count="));
            assert!(metric.contains("sum="));
            assert!(metric.contains("max="));
            assert!(metric.contains("min="));
        });

    // holochain_p2p metrics
    metrics
        .clone()
        .filter(|line| line.contains("hc.holochain_p2p.request.duration"))
        .for_each(|metric| {
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

    metrics
        .clone()
        .filter(|line| line.contains("hc.holochain_p2p.handle_request.duration"))
        .for_each(|metric| {
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

    metrics
        .clone()
        .filter(|line| line.contains("hc.holochain_p2p.recv_remote_signal"))
        .for_each(|metric| {
            assert!(metric.contains("dna_hash="));
            // Assert that received remote signal was recorded.
            assert!(metric.contains("sum=1u"));
        });

    // hc.conductor.app_ws.dropped_signal can't be easily tested, because
    // it records a metric only when signals are dropped due to channel overload.
}
