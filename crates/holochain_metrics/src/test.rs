use crate::*;
use influxive::{InfluxiveMeterProviderConfig, InfluxiveWriterConfig};
use std::{io::BufRead, time::Duration};

#[test]
fn metrics_none() {
    let config = HolochainMetricsConfig::from_env(
        std::path::PathBuf::from(".").as_path(),
        HolochainMetricsEnv::None,
    );
    assert!(matches!(config, HolochainMetricsConfig::Disabled));
}

#[tokio::test(flavor = "multi_thread")]
async fn metrics_influxive_file() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let filepath = temp_dir
        .path()
        .join(std::path::PathBuf::from("metrics.influx"));

    let config = HolochainMetricsConfig::InfluxiveFile {
        writer_config: InfluxiveWriterConfig::create_with_influx_file(
            filepath.as_path().to_path_buf(),
        ),
        otel_config: InfluxiveMeterProviderConfig::default()
            .with_report_interval(Some(Duration::from_millis(100))),
    };
    config.init().await;

    let m = opentelemetry::global::meter("test")
        .f64_histogram("my.metric")
        .build();

    // make a recording
    m.record(3.42, &[]);

    // Wait for the metric to be written
    let mut line = String::new();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let file = std::fs::File::open(&filepath).unwrap();
            let reader = std::io::BufReader::new(file);
            let res = reader.lines().next().transpose().unwrap();
            if let Some(fileline) = res {
                line = fileline;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    // Check file content for metric
    let split = line.split(' ').collect::<Vec<&str>>();
    assert_eq!(split[0], "my.metric");
    assert!(split[1].contains("3.42"));
}

#[test]
fn metrics_influxive_svc() {
    let config = HolochainMetricsConfig::from_env(
        std::path::PathBuf::from(".").as_path(),
        HolochainMetricsEnv::InfluxiveChildSvc,
    );
    assert!(matches!(
        config,
        HolochainMetricsConfig::InfluxiveChildSvc { .. }
    ));
}

#[test]
fn metrics_influxive_external() {
    let config = HolochainMetricsConfig::from_env(
        std::path::PathBuf::from(".").as_path(),
        HolochainMetricsEnv::InfluxiveExternal {
            host: String::new(),
            bucket: String::new(),
            token: String::new(),
        },
    );
    assert!(matches!(
        config,
        HolochainMetricsConfig::InfluxiveExternal { .. }
    ));
}
