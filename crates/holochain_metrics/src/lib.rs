#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(warnings)]
//! Initialize holochain metrics.
//! This crate should only be used in binaries to initialize the actual
//! metrics collection. Libraries should just use the opentelemetry_api
//! to report metrics if any collector has been initialized.

const DASH_NETWORK_STATS: &[u8] = include_bytes!("dashboards/networkstats.json");

/// Configuration for holochain metrics.
#[derive(Debug)]
pub enum HolochainMetricsConfig {
    /// Use influxive to write metrics.
    Influxive(influxive_child_svc::Config),
}

impl HolochainMetricsConfig {
    /// Initialize a new influxive metrics configuration.
    pub fn new_influxive(root_path: &std::path::Path) -> Self {
        let mut database_path = std::path::PathBuf::from(root_path);
        database_path.push("influxive");
        Self::Influxive(influxive_child_svc::Config {
            database_path: Some(database_path),
            ..Default::default()
        })
    }

    /// Initialize holochain metrics based on this configuration.
    pub async fn init(self) {
        match self {
            HolochainMetricsConfig::Influxive(config) => {
                Self::init_influxive(config).await;
            }
        }
    }

    async fn init_influxive(config: influxive_child_svc::Config) {
        tracing::info!(?config, "initializing holochain_metrics");

        match influxive_child_svc::Influxive::new(config).await {
            Ok(influxive) => {
                // apply templates
                if let Ok(cur) = influxive.list_dashboards().await {
                    // only initialize dashboards if the db is new
                    if cur.contains("\"dashboards\": []") {
                        if let Err(err) = influxive.apply(DASH_NETWORK_STATS).await {
                            tracing::warn!(?err, "failed to initialize dashboard");
                        }
                    }
                }

                // setup opentelemetry to use our metrics collector
                opentelemetry_api::global::set_meter_provider(
                    influxive_otel::InfluxiveMeterProvider::new(std::sync::Arc::new(influxive)),
                );
            }
            Err(err) => {
                tracing::warn!(?err, "unable to initialize local metrics");
            }
        }
    }
}
