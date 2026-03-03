use hdk::prelude::{Entry, EntryDef};
use holo_hash::ActionHash;
use holochain::sweettest::{
    await_consistency,  SweetConductorBatch, SweetDnaFile,
    SweetInlineZomes,
};
use holochain_types::prelude::EntryVisibility;
use holochain_zome_types::action::ChainTopOrdering;
use holochain_zome_types::entry::{EntryDefLocation};
use holochain_zome_types::fixt::AppEntryBytesFixturator;
use holochain_zome_types::prelude::{CreateInput};
use std::fs::read_to_string;
use std::time::{Duration, Instant};

#[tokio::test(flavor = "multi_thread")]
async fn metrics_test() {
    let entry_def = EntryDef::default_from_id("entry");
    let zomes =
        SweetInlineZomes::new(vec![entry_def], 1).function("create_entry", move |api, _: ()| {
            let hash = api.create(CreateInput::new(
                EntryDefLocation::app(0, 0),
                EntryVisibility::Public,
                Entry::App(::fixt::fixt!(AppEntryBytes)),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        });

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let mut conductors = SweetConductorBatch::standard(2).await;

    let tmp_file = tempfile::tempdir().unwrap();
    let influxive_file = tmp_file.path().join("metrics.influx");
    holochain_metrics::HolochainMetricsConfig::with_file(
        &influxive_file,
        Some(Duration::from_secs(1)),
    )
    .init()
    .await;

    let start = Instant::now();

    let apps = conductors.setup_app("test_app", [&dna_file]).await.unwrap();
    let cells = apps.cells_flattened();
    let alice_cell = cells.get(0).unwrap();
    let alice_zome = alice_cell.zome(SweetInlineZomes::COORDINATOR);

    // Alice creates an entry.
    let link_hash: ActionHash = conductors[0].call(&alice_zome, "create_entry", ()).await;

    await_consistency(&apps.cells_flattened()).await.unwrap();

    let seconds_elapsed = start.elapsed().as_secs() as usize;
    // It could be that the last record didn't get exported, so seconds_elapsed - 1.
    let expected_records_per_metric = seconds_elapsed - 1;

    let metrics = read_to_string(influxive_file).unwrap();
    println!("metrics:\n{metrics}\n");
    let metrics = metrics.lines();

    println!("{seconds_elapsed} s elapsed");
    println!("Expected {expected_records_per_metric} records per metric");

    // DB metrics
    let db_connections_use_time = metrics
        .clone()
        .filter(|line| line.contains("hc.db.connections.use_time"));
    let db_connections_use_time_count = db_connections_use_time.clone().count();
    // 1 record per second for 5 database kinds.
    assert!(
        db_connections_use_time_count >= expected_records_per_metric - 1 * 5,
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

    let db_pool_utilization = metrics
        .clone()
        .filter(|line| line.contains("hc.db.pool.utilization"));
    let db_pool_utilization_count = db_pool_utilization.clone().count();
    // 1 record per second for 5 database kinds
    assert!(
        db_pool_utilization_count >= expected_records_per_metric * 5,
        "expected >= {}, got {db_pool_utilization_count}",
        expected_records_per_metric * 5
    );
    db_pool_utilization.for_each(|metric| {
        assert!(metric.contains("id="));
        assert!(metric.contains("kind="));
        assert!(metric.contains("gauge="));
    });

    // Conductor metrics
    let conductor_workflow_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.conductor.workflow.duration"));
    let conductor_workflow_duration_count = conductor_workflow_duration.clone().count();
    // 1 record per second for 6 workflows
    assert!(
        conductor_workflow_duration_count >= expected_records_per_metric * 6,
        "expected >= {}, got {conductor_workflow_duration_count}",
        expected_records_per_metric * 6
    );
    conductor_workflow_duration.for_each(|metric| {
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("workflow="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });
}
