use super::*;
use std::{io::BufRead, time::Duration};

#[tokio::test(flavor = "multi_thread")]
async fn file_meter_provider_one_metric_one_value() {
    let tmp = tempfile::tempdir().unwrap();
    let test_path = tmp
        .path()
        .join(std::path::PathBuf::from("unit_test_metrics.influx"));

    // create our meter provider
    let meter_provider = influxive_file_meter_provider(
        InfluxiveWriterConfig::create_with_influx_file(test_path.clone()),
        InfluxiveMeterProviderConfig::default()
            .with_report_interval(Some(Duration::from_millis(50))),
    );

    // register our meter provider
    opentelemetry::global::set_meter_provider(meter_provider);

    // create a metric
    let m = opentelemetry::global::meter("my.meter")
        .f64_histogram("my.metric")
        .build();

    // make a recording
    m.record(4.13, &[]);

    // Wait for the metric to be written
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Check file content for metric
    let file = std::fs::File::open(&test_path).unwrap();
    let reader = std::io::BufReader::new(file);
    let res = reader.lines().next().transpose().unwrap();
    assert!(res.is_some());
    let line = res.unwrap();
    let split = line.split(' ').collect::<Vec<&str>>();
    assert_eq!(split[0], "my.metric");
    assert!(split[1].contains("4.13"));
}
