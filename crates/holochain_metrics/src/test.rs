use crate::*;

#[test]
fn metrics_none() {
    let config = HolochainMetricsConfig::from_env(
        std::path::PathBuf::from(".").as_path(),
        HolochainMetricsEnv::None,
    );
    assert!(matches!(config, HolochainMetricsConfig::Disabled));
}

#[test]
#[cfg(feature = "influxive")]
fn metrics_influxive_svc() {
    let config = HolochainMetricsConfig::from_env(
        std::path::PathBuf::from(".").as_path(),
        HolochainMetricsEnv::InfluxiveChildSvc,
    );
    assert!(matches!(
        config,
        HolochainMetricsConfig::InfluxiveChildSvc {
            child_svc_config: _,
            otel_config: _
        }
    ));
}

#[test]
#[cfg(feature = "influxive")]
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
        HolochainMetricsConfig::InfluxiveExternal {
            writer_config: _,
            otel_config: _,
            host: _,
            bucket: _,
            token: _,
        }
    ));
}
