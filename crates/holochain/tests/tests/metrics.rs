use hdk::prelude::{Entry, EntryDef, LinkTypeFilter};
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::ActionHash;
use holochain::sweettest::{
    await_consistency, SweetConductor, SweetConductorBatch, SweetConductorConfig, SweetDnaFile,
    SweetInlineZomes,
};
use holochain::test_utils::retry_fn_until_timeout;
use holochain_types::prelude::EntryVisibility;
use holochain_zome_types::action::ChainTopOrdering;
use holochain_zome_types::entry::{EntryDefLocation, GetOptions};
use holochain_zome_types::fixt::AppEntryBytesFixturator;
use holochain_zome_types::link::{CreateLinkInput, DeleteLinkInput, GetLinksInput, Link};
use holochain_zome_types::prelude::{CreateInput, LinkQuery};
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

    await_consistency(&apps.cells_flattened()).await;

    let seconds_elapsed = start.elapsed().as_secs();

    let metrics = read_to_string(influxive_file).unwrap();
    println!("metrics {metrics}");
    let metrics = metrics.lines();

    // DB metrics
    let mut hc_db_connections_use_time = metrics.clone().filter(|line|line.contains("hc.db.connections.use_time"));
    assert!(hc_db_connections_use_time.clone().count() >= seconds_elapsed as usize);
    hc_db_connections_use_time.for_each(|metric| {
        assert!(metric.contains("id="));
        assert!(metric.contains("kind="));
        assert!(metric.contains("count="));
        assert!(metric.contains("sum="));
        assert!(metric.contains("max="));
        assert!(metric.contains("min="));
    });

    let mut hc_db_pool_utilization_metric = metrics.clone().filter(|line|line.contains("hc.db.pool.utilization"));
    assert!(hc_db_pool_utilization_metric.clone().count() >= seconds_elapsed as usize);
    hc_db_pool_utilization_metric.for_each(|metric| {
        assert!(metric.contains("id="));
        assert!(metric.contains("kind="));
        assert!(metric.contains("gauge="));
    });
}
