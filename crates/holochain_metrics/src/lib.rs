#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(warnings)]
//! Initialize holochain metrics.
//! This crate should only be used in binaries to initialize the actual
//! metrics collection. Libraries should just use the opentelemetry_api
//! to report metrics if any collector has been initialized.
//!
//! ## Environment Variables
//!
//! When calling `HolochainMetricsConfig::new(&path).init()`, the actual
//! metrics instance that will be created is largely controlled by
//! the existence of environment variables.
//!
//! Curently, by default, the Null metrics collector will be used, meaning
//! metrics will not be collected, and all metrics operations will be no-ops.
//!
//! If you wish to enable metrics, the current options are:
//!
//! - InfluxDB as a zero-config child-process.
//!   - Enable via environment variable: `HOLOCHAIN_INFLUXIVE_CHILD_SVC=1`
//!   - The binaries `influxd` and `influx` will be downloaded and verified
//!     before automatically being run as a child process, and set up
//!     to be reported to. The InfluxDB UI will be available on a randomly
//!     assigned port (currently only reported in the trace logging).
//! - InfluxDB as a pre-existing system process.
//!   - Enable via environment variable: `HOLOCHAIN_INFLUXIVE_EXTERNAL=1`
//!   - Configure via environment variables:
//!     - `HOLOCHAIN_INFLUXIVE_EXTERNAL_HOST=[my influxdb url]`
//!     - `HOLOCHAIN_INFLUXIVE_EXTERNAL_BUCKET=[my influxdb bucket name]`
//!     - `HOLOCHAIN_INFLUXIVE_EXTERNAL_TOKEN=[my influxdb auth token]`
//!   - Metrics will be set up to report to this already running InfluxDB.
//!
//! ## Metric Naming Conventions
//!
//! We will largely attempt to follow the guidelines for metric naming
//! enumerated at
//! [https://opentelemetry.io/docs/specs/otel/metrics/semantic_conventions/](https://opentelemetry.io/docs/specs/otel/metrics/semantic_conventions/),
//! with additional rules made to fit with our particular project.
//! We will also attempt to keep this documentation up-to-date on a best-effort
//! basis to act as an example and registry of metrics avaliable in Holochain,
//! and related support dependency crates managed by the organization.
//!
//! Generic naming convention rules:
//!
//! - Dot notation logical module hierarchy. This need not, and perhaps should
//!   not, match the rust crate/module hierarchy. As we may rearange crates
//!   and modules, but the metric names themselves should remain more
//!   consistant.
//!   - Examples:
//!     - `hc.db`
//!     - `hc.workflow.integration`
//!     - `kitsune.gossip`
//!     - `tx5.signal`
//! - A dot notation metric name or context should follow the logical module
//!   name. The thing that can be charted should be the actual metric. Related
//!   context that may want to be filtered for the chart should be attributes.
//!   For example, a "request" may have two separate metrics, "duration", and
//!   "byte.count", which both may have the filtering attribute "remote.id".
//!   - Examples
//!     - ```
//!         use opentelemetry_api::{Context, KeyValue, metrics::Unit};
//!         let req_dur = opentelemetry_api::global::meter("tx5")
//!             .f64_histogram("tx5.signal.request.duration")
//!             .with_description("tx5 signal server request duration")
//!             .with_unit(Unit::new("s"))
//!             .init();
//!         req_dur.record(&Context::new(), 0.42, &[
//!             KeyValue::new("remote.id", "abcd"),
//!         ]);
//!       ```
//!     - ```
//!         use opentelemetry_api::{Context, KeyValue, metrics::Unit};
//!         let req_size = opentelemetry_api::global::meter("tx5")
//!             .u64_histogram("tx5.signal.request.byte.count")
//!             .with_description("tx5 signal server request byte count")
//!             .with_unit(Unit::new("By"))
//!             .init();
//!         req_size.record(&Context::new(), 42, &[
//!             KeyValue::new("remote.id", "abcd"),
//!         ]);
//!       ```
//!
//! ## Metric Name Registry
//!
//! | Full Metric Name | Type | Unit (optional) | Description | Attributes |
//! | ---------------- | ---- | --------------- | ----------- | ---------- |
//! | `kitsune.peer.send.duration` | `f64_histogram` | `s` | When kitsune sends data to a remote peer. |- `remote.id`: the base64 remote peer id.<br />- `is.error`: if the send failed. |
//! | `kitsune.peer.send.byte.count` | `u64_histogram` | `By` | When kitsune sends data to a remote peer. |- `remote.id`: the base64 remote peer id.<br />- `is.error`: if the send failed. |

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
    /// The output of this function is largely controlled by environment
    /// variables, please see the [crate-level documentation](crate) for usage.
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
                tracing::info!("Running without metrics");
            }
            #[cfg(feature = "influxive")]
            Self::InfluxiveExternal {
                config,
                host,
                bucket,
                token,
            } => {
                Self::init_influxive_external(config, host, bucket, token);
            }
            #[cfg(feature = "influxive")]
            Self::InfluxiveChildSvc(config) => {
                Self::init_influxive_child_svc(*config).await;
            }
        }
    }

    #[cfg(feature = "influxive")]
    fn init_influxive_external(
        config: influxive::InfluxiveWriterConfig,
        host: String,
        bucket: String,
        token: String,
    ) {
        tracing::info!(?config, %host, %bucket, "initializing holochain_metrics");

        let meter_provider =
            influxive::influxive_external_meter_provider_token_auth(config, host, bucket, token);

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
