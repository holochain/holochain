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
    /// Use influxive to connect to an already running InfluxDB instance.
    /// NOTE: this means we cannot initialize any dashboards.
    InfluxiveExternal {
        /// The writer config for connecting to the external influxdb instance.
        config: influxive::InfluxiveWriterConfig,

        /// The url for the external influxdb instance.
        host: String,

        /// The bucket to write to in this external influxdb instance.
        bucket: String,

        /// The authentication token to use for writing to this external
        /// influxdb instance.
        token: String,
    },

    #[cfg(feature = "influxive")]
    /// Use influxive as a child service to write metrics.
    InfluxiveChildSvc(Box<influxive::InfluxiveChildSvcConfig>),
}

const E_CHILD_SVC: &str = "HOLOCHAIN_INFLUXIVE_CHILD_SVC";

const E_EXTERNAL: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL";
const E_EXTERNAL_HOST: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL_HOST";
const E_EXTERNAL_BUCKET: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL_BUCKET";
const E_EXTERNAL_TOKEN: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL_TOKEN";

impl HolochainMetricsConfig {
    /// Initialize a new default metrics config.
    ///
    /// Seting the environment variable `HOLOCHAIN_METRICS_DISABLED` to anything
    /// will result in metrics being disabled, even if compiled with the
    /// `influxive` feature.
    pub fn new(root_path: &std::path::Path) -> Self {
        #[cfg(feature = "influxive")]
        {
            if std::env::var_os(E_CHILD_SVC).is_some() {
                let mut database_path = std::path::PathBuf::from(root_path);
                database_path.push("influxive");
                return Self::InfluxiveChildSvc(Box::new(influxive::InfluxiveChildSvcConfig {
                    database_path: Some(database_path),
                    ..Default::default()
                }));
            }

            if std::env::var_os(E_EXTERNAL).is_some() {
                let host = match std::env::var(E_EXTERNAL_HOST) {
                    Ok(host) => host,
                    Err(err) => {
                        tracing::error!(env = %E_EXTERNAL_HOST, ?err, "invalid");
                        return Self::Disabled;
                    }
                };
                let bucket = match std::env::var(E_EXTERNAL_BUCKET) {
                    Ok(bucket) => bucket,
                    Err(err) => {
                        tracing::error!(env = %E_EXTERNAL_BUCKET, ?err, "invalid");
                        return Self::Disabled;
                    }
                };
                let token = match std::env::var(E_EXTERNAL_TOKEN) {
                    Ok(token) => token,
                    Err(err) => {
                        tracing::error!(env = %E_EXTERNAL_TOKEN, ?err, "invalid");
                        return Self::Disabled;
                    }
                };
                return Self::InfluxiveExternal {
                    config: influxive::InfluxiveWriterConfig::default(),
                    host,
                    bucket,
                    token,
                };
            }
        }

        #[cfg(not(feature = "influxive"))]
        {
            let _root_path = root_path;
        }

        Self::Disabled
    }

    /// Initialize holochain metrics based on this configuration.
    pub async fn init(self) {
        match self {
            Self::Disabled => {
                tracing::warn!("Running without metrics");
            }
            #[cfg(feature = "influxive")]
            Self::InfluxiveExternal {
                config,
                host,
                bucket,
                token,
            } => {
                Self::init_influxive_external(config, host, bucket, token).await;
            }
            #[cfg(feature = "influxive")]
            Self::InfluxiveChildSvc(config) => {
                Self::init_influxive_child_svc(*config).await;
            }
        }
    }

    #[cfg(feature = "influxive")]
    async fn init_influxive_external(
        config: influxive::InfluxiveWriterConfig,
        host: String,
        bucket: String,
        token: String,
    ) {
        tracing::info!(?config, %host, %bucket, "initializing holochain_metrics");

        let meter_provider =
            influxive::influxive_external_meter_provider_token_auth(config, host, bucket, token)
                .await;

        // setup opentelemetry to use our metrics collector
        opentelemetry_api::global::set_meter_provider(meter_provider);
    }

    #[cfg(feature = "influxive")]
    async fn init_influxive_child_svc(config: influxive::InfluxiveChildSvcConfig) {
        tracing::info!(?config, "initializing holochain_metrics");

        match influxive::influxive_child_process_meter_provider(config).await {
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
