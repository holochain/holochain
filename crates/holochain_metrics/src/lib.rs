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
    /// Metrics are disabled.
    Disabled,

    #[cfg(feature = "influxive")]
    /// Use influxive to write metrics.
    Influxive(Box<influxive::Config>),
}

impl HolochainMetricsConfig {
    /// Initialize a new default metrics config.
    ///
    /// Seting the environment variable `HOLOCHAIN_METRICS_DISABLED` to anything
    /// will result in metrics being disabled, even if compiled with the
    /// `influxive` feature.
    pub fn new(root_path: &std::path::Path) -> Self {
        #[cfg(feature = "influxive")]
        {
            if std::env::var_os("HOLOCHAIN_METRICS_DISABLED").is_some() {
                Self::Disabled
            } else {
                let mut database_path = std::path::PathBuf::from(root_path);
                database_path.push("influxive");
                Self::Influxive(Box::new(influxive::Config {
                    database_path: Some(database_path),
                    ..Default::default()
                }))
            }
        }

        #[cfg(not(feature = "influxive"))]
        {
            let _root_path = root_path;
            Self::Disabled
        }
    }

    /// Initialize holochain metrics based on this configuration.
    pub async fn init(self) {
        match self {
            Self::Disabled => {
                tracing::warn!("Running without metrics");
            }
            #[cfg(feature = "influxive")]
            Self::Influxive(config) => {
                Self::init_influxive(*config).await;
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
