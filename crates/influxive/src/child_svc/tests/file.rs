use crate::child_svc::{InfluxiveChildSvc, InfluxiveChildSvcConfig};
use crate::types::*;
use crate::writer::*;
use std::path::{Path, PathBuf};
use std::time::Duration;
use telegraf_influx_file_conf::TelegrafLineProtocolConfig;
use telegraf_svc::TelegrafSvc;

mod telegraf_binaries;
mod telegraf_influx_file_conf;
mod telegraf_svc;

/// Setup [`InfluxiveWriter`] to use [`LineProtocolFileBackendFactory`]
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
    let telegraf_config_path = test_dir.join("test_telegraf.conf");

    // Write metrics to disk
    write_metrics_to_file(&metrics_path).await;

    // Launch influxDB
    let influx_process = spawn_influx(&test_dir).await;

    // Generate Telegraf config
    let config = TelegrafLineProtocolConfig::new(
        influx_process.get_host(),
        influx_process.get_token(),
        "influxive",
        "influxive",
        metrics_path.to_str().unwrap(),
    );
    assert!(config.write_to_file(telegraf_config_path.as_path()).is_ok());

    // Launch Telegraf
    let _telegraf_process = TelegrafSvc::spawn(
        telegraf_config_path.to_str().unwrap(),
        test_dir.to_str().unwrap(),
        true,
    )
    .await
    .unwrap();

    // Wait for telegraf to process by querying influxDB every second until we get the expected
    // result or a timeout
    let mut line_count = 0;
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let result = influx_process
                .query(
                    r#"from(bucket: "influxive")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r["_measurement"] == "my-second-metric")
|> filter(fn: (r) => r["_field"] == "val")"#,
                )
                .await
                .unwrap();

            line_count = result
                .split('\n')
                .filter(|l| l.contains("my-second-metric"))
                .count();
            if line_count == 10 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("Error: Test timed out. line_count = {line_count} ; Expected: 10"));
}
