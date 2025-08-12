use crate::*;
use std::io::BufRead;

#[test]
fn metrics_file_config_bad_env() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let config = HolochainMetricsConfig::new(temp_dir.as_ref());
    assert!(matches!(config, HolochainMetricsConfig::Disabled));

    let config = temp_env::with_var("HOLOCHAIN_INFLUXIVE_FILE", Some("0"), || {
        HolochainMetricsConfig::new(temp_dir.as_ref())
    });
    assert!(matches!(config, HolochainMetricsConfig::Disabled));

    let config = temp_env::with_vars(
        [
            ("HOLOCHAIN_INFLUXIVE_FILE", Some("1")),
            ("HOLOCHAIN_INFLUXIVE_FILE_PATH", None),
        ],
        || HolochainMetricsConfig::new(temp_dir.as_ref()),
    );
    assert!(matches!(config, HolochainMetricsConfig::Disabled));

    let config = temp_env::with_vars(
        [
            ("HOLOCHAIN_INFLUXIVE_FILE", Some("1")),
            ("HOLOCHAIN_INFLUXIVE_FILE_PATH", Some("")),
        ],
        || HolochainMetricsConfig::new(temp_dir.as_ref()),
    );
    assert!(matches!(config, HolochainMetricsConfig::Disabled));
}

#[tokio::test(flavor = "multi_thread")]
async fn metrics_file_config_ok() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let filepath = temp_dir
        .path()
        .join(std::path::PathBuf::from("metrics.influx"));

    let config = temp_env::with_vars(
        [
            ("HOLOCHAIN_INFLUXIVE_FILE", Some("1")),
            (
                "HOLOCHAIN_INFLUXIVE_FILE_PATH",
                Some(filepath.to_str().unwrap()),
            ),
        ],
        || HolochainMetricsConfig::new(temp_dir.as_ref()),
    );
    assert!(matches!(
        config,
        HolochainMetricsConfig::InfluxiveFile {
            writer_config: _,
            otel_config: _
        }
    ));

    config.init().await;

    let m = opentelemetry_api::global::meter("test")
        .f64_histogram("my.metric")
        .init();

    // make a recording
    m.record(3.42, &[]);

    // Wait for the metric to be written
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Check file content for metric
    let file = std::fs::File::open(&filepath).unwrap();
    let reader = std::io::BufReader::new(file);
    let res = reader.lines().next().transpose().unwrap();
    assert!(res.is_some());
    let line = res.unwrap();
    let split = line.split(' ').collect::<Vec<&str>>();
    assert_eq!(split[0], "my.metric");
    assert!(split[1].contains("3.42"));
}
