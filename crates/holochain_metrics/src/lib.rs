#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(warnings)]
//! Initialize holochain metrics.
//! This crate should only be used in binaries to initialize the actual
//! metrics collection. Libraries should just use the opentelemetry_api
//! to report metrics if any collector has been initialized.

#[cfg(feature = "influxive")]
const DASH_NETWORK_STATS: &[u8] = include_bytes!("dashboards/networkstats.json");

/// Configuration for holochain metrics.
#[derive(Debug)]
pub enum HolochainMetricsConfig {
    #[cfg(feature = "influxive")]
    /// Use influxive to write metrics.
    Influxive(influxive::Config),
}

impl HolochainMetricsConfig {
    #[cfg(feature = "influxive")]
    /// Initialize a new influxive metrics configuration.
    pub fn new_influxive(root_path: &std::path::Path) -> Self {
        let mut database_path = std::path::PathBuf::from(root_path);
        database_path.push("influxive");
        Self::Influxive(influxive::Config {
            database_path: Some(database_path),
            ..Default::default()
        })
    }

    /// Initialize holochain metrics based on this configuration.
    pub async fn init(self) {
        match self {
            #[cfg(feature = "influxive")]
            HolochainMetricsConfig::Influxive(config) => {
                Self::init_influxive(config).await;
            }
        }
    }

    #[cfg(feature = "influxive")]
    async fn init_influxive(config: influxive::Config) {
        tracing::info!(?config, "initializing holochain_metrics");

        match influxive::influxive_meter_provider(config).await {
            Ok((influxive, meter_provider)) => {
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
                opentelemetry_api::global::set_meter_provider(meter_provider);

                tracing::info!(host = %influxive.get_host(), "influxive metrics running");
            }
            Err(err) => {
                tracing::warn!(?err, "unable to initialize local metrics");
            }
        }
    }
}
