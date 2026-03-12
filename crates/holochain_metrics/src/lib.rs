#![deny(missing_docs)]
#![deny(unsafe_code)]
//! Initialize holochain metrics.
//! This crate should only be used in binaries to initialize the actual
//! metrics collection. Libraries should just use the opentelemetry crate
//! to report metrics if any collector has been initialized.
//!
//! ## Environment Variables
//!
//! When calling `HolochainMetricsConfig::new(&path).init()`, the actual
//! metrics instance that will be created is largely controlled by
//! the existence of environment variables.
//!
//! Currently, by default, the Null metrics collector will be used, meaning
//! metrics will not be collected, and all metrics operations will be no-ops.
//!
//! If you wish to enable metrics, the current options are:
//!
//! - A file, containing InfluxDB line protocol metrics. These can be pushed to InfluxDB later with Telegraf.
//!   - Enable and configure via environment variable: `HOLOCHAIN_INFLUXIVE_FILE="path/to/influx/file"`
//! - InfluxDB as a zero-config child-process.
//!   - Enable via environment variable: `HOLOCHAIN_INFLUXIVE_CHILD_SVC=1`
//!   - The binaries `influxd` and `influx` will be downloaded and verified
//!     before automatically being run as a child process, and set up
//!     to be reported to. The InfluxDB UI will be available on a randomly
//!     assigned port (currently only reported in the trace logging).
//! - InfluxDB as a pre-existing system process.
//!   - Enable via environment variable: `HOLOCHAIN_INFLUXIVE_EXTERNAL=1`
//!   - Configure via environment variables:
//!     - `HOLOCHAIN_INFLUXIVE_EXTERNAL_HOST=[my influxdb url]` where a default InfluxDB install will need `http://localhost:8086` and otherwise can be found by running `influx config` in a terminal.
//!     - `HOLOCHAIN_INFLUXIVE_EXTERNAL_BUCKET=[my influxdb bucket name]` but it's simplest to use `influxive` if you plan to import the provided dashboards.
//!     - `HOLOCHAIN_INFLUXIVE_EXTERNAL_TOKEN=[my influxdb auth token]`
//!   - The influxdb auth token must have permission to write to all buckets
//!   - Metrics will be set up to report to this already running InfluxDB.
//!
//! To set the interval at which recorded metrics are written to Influx,
//! use `OTEL_METRIC_EXPORT_INTERVAL`. The value is specified as milliseconds.
//! 10 s is the default. When the report interval is configured in the code,
//! it overrides this environment variable setting.
//!
//! ## Metric Naming Conventions
//!
//! We will largely attempt to follow the guidelines for metric naming
//! enumerated at
//! [https://opentelemetry.io/docs/specs/otel/metrics/semantic_conventions/](https://opentelemetry.io/docs/specs/otel/metrics/semantic_conventions/),
//! with additional rules made to fit with our particular project.
//! We will also attempt to keep this documentation up to date on a best-effort
//! basis to act as an example and registry of metrics available in Holochain,
//! and related support dependency crates managed by the organization.
//!
//! Generic naming convention rules:
//!
//! - Dot notation logical module hierarchy. This need not, and perhaps should
//!   not, match the rust crate/module hierarchy. As we may rearrange crates
//!   and modules, but the metric names themselves should remain more
//!   consistent.
//!   - Examples:
//!     - `hc.db`
//!     - `hc.workflow.integration`
//!     - `hc.ribosome.wasm`
//! - A dot notation metric name or context should follow the logical module
//!   name. The thing that can be charted should be the actual metric. Related
//!   context that may want to be filtered for the chart should be attributes.
//!   For example, a "request" may have two separate metrics, "duration", and
//!   "byte.count", which both may have the filtering attribute "remote_id".
//!   - Examples
//!     - ```
//!         use opentelemetry::KeyValue;
//!         let req_dur = opentelemetry::global::meter("hc")
//!             .f64_histogram("hc.holochain_p2p.request.duration")
//!             .with_description("holochain p2p request duration")
//!             .with_unit("s")
//!             .build();
//!         req_dur.record(0.42, &[KeyValue::new("remote_id", "abcd")]);
//!       ```
//!     - ```
//!         use opentelemetry::KeyValue;
//!         let req_size = opentelemetry::global::meter("hc")
//!             .u64_histogram("hc.holochain_p2p.request.byte.count")
//!             .with_description("holochain p2p request byte count")
//!             .with_unit("B")
//!             .build();
//!         req_size.record(42, &[
//!             KeyValue::new("remote_id", "abcd"),
//!         ]);
//!       ```
//!
//! ## Metric Name Registry
//!
//! These following metrics are defined and recorded in their respective crates.
//! Do a text search to look up metric type, description and unit.
//!
//! | Full Metric Name | Type | Unit (optional) | Description | Attributes |
//! | ---------------- | ---- | --------------- | ----------- | ---------- |
//! | `hc.db.connections.use_time` | f64 histogram | s | The time between borrowing a connection and returning it to the pool | `kind`: DB type (authored/dht/cache/…), `id`: DB instance identifier |
//! | `hc.db.write_txn.duration` | f64 histogram | s | The time spent executing an exclusive write transaction | `kind`: DB type (authored/dht/cache/…), `id`: DB instance identifier |
//! | `hc.keystore.lair_request.duration` | f64 histogram | s | Duration of signing and encryption requests to Lair | `operation`: cryptographic operation (sign/encrypt/…) |
//! | `hc.conductor.workflow.duration` | f64 histogram | s | The time spent running a workflow | `workflow`: workflow process name, `dna_hash`: DNA identifier, `agent`: agent public key |
//! | `hc.conductor.workflow.integrated_ops` | u64 counter | | The number of integrated operations | |
//! | `hc.conductor.workflow.integration_delay` | f64 histogram | s | Time between an op being stored and it being integrated | |
//! | `hc.conductor.workflow.validation_attempts` | u64 histogram | | Number of validation attempts required to integrate an op | |
//! | `hc.conductor.post_commit.duration` | f64 histogram | s | The time spent executing a post commit | `dna_hash`: DNA identifier, `agent`: agent public key |
//! | `hc.conductor.uptime` | f64 observable gauge | s | The number of seconds the conductor has been running | |
//! | `hc.conductor.app_ws.dropped_signal` | u64 counter | | The number of signals dropped from app ws due to channel overload | |
//! | `hc.ribosome.wasm.usage` | u64 counter | | The metered usage of a wasm ribosome | `dna_hash`: DNA identifier, `zome`: zome module name, `fn`: function name, `agent`: agent public key |
//! | `hc.ribosome.zome_call.duration` | f64 histogram | s | The time spent running a zome call | `dna_hash`: DNA identifier, `zome`: zome module name, `fn`: function name |
//! | `hc.ribosome.wasm_call.duration` | f64 histogram | s | The time spent running a wasm call | `dna_hash`: DNA identifier, `zome`: zome module name, `fn`: function name, `agent`: agent public key |
//! | `hc.ribosome.host_fn_call.duration` | f64 histogram | s | The time spent executing a host function call | `dna_hash`: DNA identifier, `zome`: zome module name, `fn`: function name, `host_fn`: host function name |
//! | `hc.ribosome.host_fn.emit_signal` | u64 counter | | The number of local signals emitted | `cell_id`: cell identifier, `zome`: zome module name |
//! | `hc.ribosome.host_fn.send_remote_signal` | u64 counter | | The number of remote signals sent | `dna_hash`: DNA identifier, `zome`: zome module name |
//! | `hc.cascade.duration` | f64 histogram | s | The time taken to execute a cascade query | `zome`: originating zome name, `fn`: originating function name |
//! | `hc.cascade.fetch_error` | u64 counter | | Number of errors encountered while fetching data from the network | `fetch_type`: type of data fetched, `zome`: originating zome name, `fn`: originating function name |
//! | `hc.holochain_p2p.request.duration` | f64 histogram | s | The time spent sending an outgoing p2p request awaiting the response | `dna_hash`: DNA identifier, `tag`: request category tag, `error`: request failed, `zome`: originating zome name, `fn`: originating function name |
//! | `hc.holochain_p2p.handle_request.duration` | f64 histogram | s | The time spent handling an incoming p2p request | `message_type`: p2p message type, `dna_hash`: DNA identifier |
//! | `hc.holochain_p2p.recv_remote_signal` | u64 counter | | The number of remote signals received | `dna_hash`: DNA identifier |

use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) mod influxive;

#[cfg(test)]
mod test;

const DASH_DATABASE: &[u8] = include_bytes!("dashboards/database.json");

const DASH_CONDUCTOR: &[u8] = include_bytes!("dashboards/conductor.json");

const DASH_WASM: &[u8] = include_bytes!("dashboards/wasm.json");

const VAR_CELL_ID: &[u8] = include_bytes!("variables/cellid.json");

/// Configuration for holochain metrics set by environment variables.
enum HolochainMetricsEnv {
    None,

    InfluxiveFile {
        filepath: String,
    },

    InfluxiveChildSvc,

    InfluxiveExternal {
        host: String,
        bucket: String,
        token: String,
    },
}

impl HolochainMetricsEnv {
    pub fn load() -> Self {
        // Environment variable to set for enabling metrics with influxDB run as a child service.
        const ENV_CHILD_SVC: &str = "HOLOCHAIN_INFLUXIVE_CHILD_SVC";

        // Environment variable to set for enabling metrics with an externally running influxDB.
        const ENV_EXTERNAL: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL";
        // Environment variable of the external influxDB host to use.
        const ENV_EXTERNAL_HOST: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL_HOST";
        // Environment variable of the influxDB bucket to use.
        const ENV_EXTERNAL_BUCKET: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL_BUCKET";
        // Environment variable of the influxDB token to use.
        const ENV_EXTERNAL_TOKEN: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL_TOKEN";

        // Environment variable to set for enabling metrics to a file on disk.
        const ENV_FILE: &str = "HOLOCHAIN_INFLUXIVE_FILE";

        if let Some(filepath) = std::env::var_os(ENV_FILE) {
            return Self::InfluxiveFile {
                filepath: filepath.to_string_lossy().to_string(),
            };
        }

        if std::env::var_os(ENV_CHILD_SVC).is_some() {
            return Self::InfluxiveChildSvc;
        };

        if std::env::var_os(ENV_EXTERNAL).is_some() {
            let host = match std::env::var(ENV_EXTERNAL_HOST) {
                Ok(host) => host,
                Err(err) => {
                    tracing::error!(env = %ENV_EXTERNAL_HOST, ?err, "invalid");
                    return Self::None;
                }
            };
            let bucket = match std::env::var(ENV_EXTERNAL_BUCKET) {
                Ok(bucket) => bucket,
                Err(err) => {
                    tracing::error!(env = %ENV_EXTERNAL_BUCKET, ?err, "invalid");
                    return Self::None;
                }
            };
            let token = match std::env::var(ENV_EXTERNAL_TOKEN) {
                Ok(token) => token,
                Err(err) => {
                    tracing::error!(env = %ENV_EXTERNAL_TOKEN, ?err, "invalid");
                    return Self::None;
                }
            };
            return Self::InfluxiveExternal {
                host,
                bucket,
                token,
            };
        }
        Self::None
    }
}

/// Configuration for holochain metrics.
pub enum HolochainMetricsConfig {
    /// Metrics are disabled.
    Disabled,

    /// Use influxive to write metrics to a file.
    ///
    /// NOTE: this means Holochain cannot initialize dashboards because it won't know where your
    /// InfluxDB server is or have credentials for it.
    InfluxiveFile {
        /// The writer config for writing metrics to a file.
        writer_config: influxive::InfluxiveWriterConfig,
        /// The meter provider config for setting up opentelemetry.
        otel_config: influxive::InfluxiveMeterProviderConfig,
    },

    /// Use influxive to connect to an already running InfluxDB instance.
    /// NOTE: this means we cannot initialize any dashboards.
    InfluxiveExternal {
        /// The writer config for connecting to the external influxdb instance.
        writer_config: influxive::InfluxiveWriterConfig,

        /// The meter provider config for setting up opentelemetry.
        otel_config: influxive::InfluxiveMeterProviderConfig,

        /// The url for the external influxdb instance.
        host: String,

        /// The bucket to write to in this external influxdb instance.
        bucket: String,

        /// The authentication token to use for writing to this external
        /// influxdb instance.
        token: String,
    },

    /// Use influxive as a child service to write metrics.
    InfluxiveChildSvc {
        /// The child service config for running the influxd server.
        child_svc_config: Box<influxive::InfluxiveChildSvcConfig>,

        /// The meter provider config for setting up opentelemetry.
        otel_config: influxive::InfluxiveMeterProviderConfig,
    },
}

impl HolochainMetricsConfig {
    /// Initialize a new default metrics config.
    ///
    /// The output of this function is largely controlled by environment
    /// variables, please see the [crate-level documentation](crate) for usage.
    pub fn new_from_env_vars(root_path: &Path) -> Self {
        Self::from_env(root_path, HolochainMetricsEnv::load())
    }

    /// Construct a config with an Influxive file.
    pub fn new_with_file(
        file_path: &Path,
        report_interval: Option<Duration>,
    ) -> HolochainMetricsConfig {
        HolochainMetricsConfig::InfluxiveFile {
            writer_config: influxive::InfluxiveWriterConfig::create_with_influx_file(
                PathBuf::from(file_path),
            ),
            otel_config: influxive::InfluxiveMeterProviderConfig::default()
                .with_report_interval(report_interval),
        }
    }

    fn from_env(root_path: &Path, env: HolochainMetricsEnv) -> Self {
        match env {
            HolochainMetricsEnv::InfluxiveFile { filepath } => {
                Self::new_with_file(Path::new(&filepath), None)
            }

            HolochainMetricsEnv::InfluxiveChildSvc => {
                let mut database_path = PathBuf::from(root_path);
                database_path.push("influxive");
                Self::InfluxiveChildSvc {
                    child_svc_config: Box::new(
                        influxive::InfluxiveChildSvcConfig::default()
                            .with_database_path(Some(database_path)),
                    ),
                    otel_config: influxive::InfluxiveMeterProviderConfig::default(),
                }
            }

            HolochainMetricsEnv::InfluxiveExternal {
                host,
                bucket,
                token,
            } => Self::InfluxiveExternal {
                writer_config: influxive::InfluxiveWriterConfig::default(),
                otel_config: influxive::InfluxiveMeterProviderConfig::default(),
                host,
                bucket,
                token,
            },
            HolochainMetricsEnv::None => Self::Disabled,
        }
    }

    /// Initialize holochain metrics based on this configuration.
    pub async fn init(self) {
        match self {
            Self::Disabled => {
                tracing::info!("Running without metrics");
            }

            Self::InfluxiveFile {
                writer_config,
                otel_config,
            } => {
                Self::init_influxive_file(writer_config, otel_config);
            }

            Self::InfluxiveExternal {
                writer_config,
                otel_config,
                host,
                bucket,
                token,
            } => {
                Self::init_influxive_external(writer_config, otel_config, host, bucket, token);
            }
            Self::InfluxiveChildSvc {
                child_svc_config,
                otel_config,
            } => {
                Self::init_influxive_child_svc(*child_svc_config, otel_config).await;
            }
        }
    }

    fn init_influxive_file(
        writer_config: influxive::InfluxiveWriterConfig,
        otel_config: influxive::InfluxiveMeterProviderConfig,
    ) {
        tracing::info!(
            ?writer_config,
            "initializing holochain_metrics for file output"
        );

        let meter_provider = influxive::influxive_file_meter_provider(writer_config, otel_config);

        // set up opentelemetry to use our metrics collector
        opentelemetry::global::set_meter_provider(meter_provider);
    }

    fn init_influxive_external(
        writer_config: influxive::InfluxiveWriterConfig,
        otel_config: influxive::InfluxiveMeterProviderConfig,
        host: String,
        bucket: String,
        token: String,
    ) {
        tracing::info!(?writer_config, %host, %bucket, "initializing holochain_metrics");

        let meter_provider = influxive::influxive_external_meter_provider_token_auth(
            writer_config,
            otel_config,
            host,
            bucket,
            token,
        );

        // setup opentelemetry to use our metrics collector
        opentelemetry::global::set_meter_provider(meter_provider);
    }

    async fn init_influxive_child_svc(
        child_svc_config: influxive::InfluxiveChildSvcConfig,
        otel_config: influxive::InfluxiveMeterProviderConfig,
    ) {
        tracing::info!(?child_svc_config, "initializing holochain_metrics");

        match influxive::influxive_child_process_meter_provider(child_svc_config, otel_config).await
        {
            Ok((influxive, meter_provider)) => {
                // apply templates if the db is new
                if let Ok(cur) = influxive.list_dashboards().await {
                    if cur.contains("\"dashboards\": []") {
                        if let Err(err) = influxive.apply(DASH_DATABASE).await {
                            tracing::warn!(?err, "failed to initialize database dashboard");
                        }
                        if let Err(err) = influxive.apply(DASH_CONDUCTOR).await {
                            tracing::warn!(?err, "failed to initialize conductor dashboard");
                        }
                        if let Err(err) = influxive.apply(DASH_WASM).await {
                            tracing::warn!(?err, "failed to initialize wasm dashboard");
                        }
                        if let Err(err) = influxive.apply(VAR_CELL_ID).await {
                            tracing::warn!(?err, "failed to initialize CellId variable");
                        }
                    }
                }

                // setup opentelemetry to use our metrics collector
                opentelemetry::global::set_meter_provider(meter_provider);

                tracing::info!(host = %influxive.get_host(), "influxive metrics running");
            }
            Err(err) => {
                tracing::warn!(?err, "unable to initialize local metrics");
            }
        }
    }
}
