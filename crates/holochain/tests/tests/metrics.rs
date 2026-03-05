use futures::FutureExt;
use hdk::prelude::{Entry, EntryDef, Record};
use holo_hash::ActionHash;
use holochain::sweettest::{
    await_consistency, SweetConductorBatch, SweetDnaFile, SweetInlineZomes,
};
use holochain_types::prelude::EntryVisibility;
use holochain_zome_types::action::ChainTopOrdering;
use holochain_zome_types::entry::EntryDefLocation;
use holochain_zome_types::fixt::AppEntryBytesFixturator;
use holochain_zome_types::prelude::{CreateInput, GetInput, GetOptions};
use std::fs::read_to_string;
use std::time::{Duration, Instant};

#[tokio::test(flavor = "multi_thread")]
async fn metrics() {
    let entry_def = EntryDef::default_from_id("entry");
    let zomes = SweetInlineZomes::new(vec![entry_def], 1)
        .function("create_entry", move |api, _: ()| {
            let hash = api.create(CreateInput::new(
                EntryDefLocation::app(0, 0),
                EntryVisibility::Public,
                Entry::App(::fixt::fixt!(AppEntryBytes)),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("get_entry", move |api, hash: ActionHash| {
            Ok(api.get(vec![GetInput::new(hash.into(), GetOptions::default())])?)
        });

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    let tmp_file = tempfile::tempdir().unwrap();
    let influxive_file = tmp_file.path().join("metrics.influx");
    holochain_metrics::HolochainMetricsConfig::with_file(
        &influxive_file,
        Some(Duration::from_secs(1)),
    )
    .init()
    .await;

    let mut conductors = SweetConductorBatch::standard(2).await;

    let start = Instant::now();
    let mut get_requests = 0;

    let apps = conductors.setup_app("test_app", [&dna_file]).await.unwrap();
    let alice_conductor = conductors.get(0).unwrap();
    let bob_conductor = conductors.get(1).unwrap();
    let cells = apps.cells_flattened();
    let alice_cell = cells.get(0).unwrap();
    let alice_zome = alice_cell.zome(SweetInlineZomes::COORDINATOR);
    let bob_cell = cells.get(1).unwrap();
    let bob_zome = bob_cell.zome(SweetInlineZomes::COORDINATOR);

    // Alice creates an entry.
    let create_entry_hash: ActionHash = alice_conductor.call(&alice_zome, "create_entry", ()).await;
    // Bob gets Alice's entry.
    let _: Vec<Option<Record>> = bob_conductor
        .call(&bob_zome, "get_entry", create_entry_hash.clone())
        .await;
    get_requests += 1;

    await_consistency(&apps.cells_flattened()).await.unwrap();

    let seconds_elapsed = start.elapsed().as_secs() as usize;
    // It could be that the last record didn't get exported, so seconds_elapsed - 1.
    let expected_records_per_metric = seconds_elapsed - 1;
    println!("{seconds_elapsed} s elapsed");
    println!("Expected {expected_records_per_metric} records per metric");
    println!();

    let metrics = read_to_string(influxive_file).unwrap();
    println!("{metrics}");
    let metrics = metrics.lines();

    // db metrics
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

    // conductor metrics
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

    // cascade metrics
    let cascade_duration = metrics
        .clone()
        .filter(|line| line.contains("hc.cascade.duration"));
    let cascade_duration_count = cascade_duration.clone().count();
    // 1 record per get request
    assert!(
        cascade_duration_count >= get_requests,
        "expected >= 1, got {cascade_duration_count}",
    );
    cascade_duration.for_each(|metric| {
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
        "expected >= 1, got {request_duration_count}",
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
        "expected >= 1, got {handle_request_duration_count}",
    );
    handle_request_duration.for_each(|metric| {
        assert!(metric.contains("message_type="));
        assert!(metric.contains("dna_hash="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    // hc.holochain_p2p.handle_request.ignored can't be easily tested


}
