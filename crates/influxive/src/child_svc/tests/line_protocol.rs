use crate::child_svc::{InfluxiveChildSvc, InfluxiveChildSvcConfig};
use crate::types::*;
use crate::writer::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Setup [`InfluxiveWriter`]
pub fn create_influx_file_writer(test_path: &PathBuf) -> InfluxiveWriter {
    let _ = std::fs::remove_file(test_path);
    let mut config = InfluxiveWriterConfig::create_with_influx_file(test_path.clone());
    config.batch_duration = std::time::Duration::from_millis(30);
    InfluxiveWriter::with_token_auth(config.clone(), "", "", "")
}

/// Spawn influxDB with the default config
async fn spawn_influx(path: &Path) -> InfluxiveChildSvc {
    let child = InfluxiveChildSvc::new(
        InfluxiveChildSvcConfig::default()
            .with_database_path(Some(path.into()))
            .with_metric_write(
                InfluxiveWriterConfig::default()
                    .with_batch_duration(std::time::Duration::from_millis(5)),
            ),
    )
    .await
    .unwrap();

    child.ping().await.unwrap();

    child
}

async fn write_metrics_to_file(test_path: &PathBuf) {
    use std::io::BufRead;

    let writer = create_influx_file_writer(test_path);

    // Write one metric
    writer.write_metric(
        Metric::new(std::time::SystemTime::now(), "my-metric")
            .with_field("f1", 1.77)
            .with_field("f2", 2.77)
            .with_field("f3", 3.77)
            .with_tag("tag", "test-tag")
            .with_tag("tag2", "test-tag2"),
    );

    // Write many metrics with different timestamps
    let now = std::time::SystemTime::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap();
    for n in 0..10 {
        writer.write_metric(
            Metric::new(
                now.checked_add(Duration::from_secs(n)).unwrap(),
                "my-second-metric",
            )
            .with_field("val", n),
        );
    }

    // Wait for batch processing to trigger
    tokio::time::timeout(std::time::Duration::from_millis(1000), async {
        loop {
            // Make sure metrics have been written to disk
            let file = std::fs::File::open(test_path).unwrap();
            let reader = std::io::BufReader::new(file);
            let count = reader.lines().count();
            if count == 11 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn write_to_file_then_read() {
    let test_dir_guard = tempfile::tempdir().unwrap();
    let test_dir = test_dir_guard.path().to_owned();
    let metrics_path = test_dir.join("test_metrics.influx");

    // Write test metrics to disk in line protocol format
    write_metrics_to_file(&metrics_path).await;

    // Launch influxDB
    let influx_process = spawn_influx(&test_dir).await;

    // Read the line protocol file and write directly to InfluxDB
    let line_protocol = std::fs::read_to_string(&metrics_path).unwrap();
    influx_process
        .write_line_protocol(&line_protocol)
        .await
        .unwrap();

    // Query influxDB to verify the data was written correctly
    let result = influx_process
        .query(
            r#"from(bucket: "influxive")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r["_measurement"] == "my-second-metric")
|> filter(fn: (r) => r["_field"] == "val")"#,
        )
        .await
        .unwrap();

    let line_count = result
        .split('\n')
        .filter(|l| l.contains("my-second-metric"))
        .count();

    assert_eq!(line_count, 10, "Expected 10 metrics, got {line_count}");
}
