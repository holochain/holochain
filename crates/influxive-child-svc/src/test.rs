use super::*;

const DASHBOARD_TEMPLATE: &[u8] = include_bytes!("test_dashboard_template.json");

#[tokio::test(flavor = "multi_thread")]
async fn sanity() {
    let tmp = tempfile::tempdir().unwrap();

    const METRIC: &str = "my.metric";
    const VALUE: &str = "value";

    let i = InfluxiveChildSvc::new(
        InfluxiveChildSvcConfig::default()
            .with_database_path(Some(tmp.path().into()))
            .with_metric_write(
                InfluxiveWriterConfig::default()
                    .with_batch_duration(std::time::Duration::from_millis(5)),
            ),
    )
    .await
    .unwrap();

    println!("{}", i.get_host());

    i.ping().await.unwrap();

    println!("{}", i.list_dashboards().await.unwrap());
    println!("{}", i.apply(DASHBOARD_TEMPLATE).await.unwrap());
    println!("{}", i.list_dashboards().await.unwrap());

    let mut last_time = std::time::Instant::now();

    for _ in 0..12 {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        i.write_metric(
            Metric::new(std::time::SystemTime::now(), METRIC)
                .with_field(VALUE, last_time.elapsed().as_secs_f64())
                .with_tag("tag-name", "tag-value"),
        );

        last_time = std::time::Instant::now();
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let result = i
        .query(
            r#"from(bucket: "influxive")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r["_measurement"] == "my.metric")
|> filter(fn: (r) => r["_field"] == "value")"#,
        )
        .await
        .unwrap();

    // make sure the result contains at least 10 of the entries
    let line_count = result.split('\n').count();
    assert!(line_count >= 10, "{result}");

    drop(i);

    // okay if this fails on windows...
    let _ = tmp.close();
}
