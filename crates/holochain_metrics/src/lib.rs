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
//!     - `HOLOCHAIN_INFLUXIVE_EXTERNAL_HOST=[my influxdb url]` where a default InfluxDB install will need `http://localhost:8086` and otherwise can be found by running `influx config` in a terminal.
//!     - `HOLOCHAIN_INFLUXIVE_EXTERNAL_BUCKET=[my influxdb bucket name]` but it's simplest to use `influxive` if you plan to import the provided dashboards.
//!     - `HOLOCHAIN_INFLUXIVE_EXTERNAL_TOKEN=[my influxdb auth token]`
//!   - The influxdb auth token must have permission to write to all buckets
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
//!   "byte.count", which both may have the filtering attribute "remote_id".
//!   - Examples
//!     - ```
//!         use opentelemetry_api::{Context, KeyValue, metrics::Unit};
//!         let req_dur = opentelemetry_api::global::meter("tx5")
//!             .f64_histogram("tx5.signal.request.duration")
//!             .with_description("tx5 signal server request duration")
//!             .with_unit(Unit::new("s"))
//!             .init();
//!         req_dur.record(&Context::new(), 0.42, &[
//!             KeyValue::new("remote_id", "abcd"),
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
//!             KeyValue::new("remote_id", "abcd"),
//!         ]);
//!       ```
//!
//! ## Metric Name Registry
//!
//! | Full Metric Name | Type | Unit (optional) | Description | Attributes |
//! | ---------------- | ---- | --------------- | ----------- | ---------- |
//! | `kitsune.peer.send.duration` | `f64_histogram` | `s` | When kitsune sends data to a remote peer. |- `remote_id`: the base64 remote peer id.<br />- `is_error`: if the send failed. |
//! | `kitsune.peer.send.byte.count` | `u64_histogram` | `By` | When kitsune sends data to a remote peer. |- `remote_id`: the base64 remote peer id.<br />- `is_error`: if the send failed. |
//! | `tx5.conn.ice.send` | `u64_observable_counter` | `By` | Bytes sent on ice channel. |- `remote_id`: the base64 remote peer id.<br />- `state_uniq`: endpoint identifier.<br />- `conn_uniq`: connection identifier. |
//! | `tx5.conn.ice.recv` | `u64_observable_counter` | `By` | Bytes received on ice channel. |- `remote_id`: the base64 remote peer id.<br />- `state_uniq`: endpoint identifier.<br />- `conn_uniq`: connection identifier. |
//! | `tx5.conn.data.send` | `u64_observable_counter` | `By` | Bytes sent on data channel. |- `remote_id`: the base64 remote peer id.<br />- `state_uniq`: endpoint identifier.<br />- `conn_uniq`: connection identifier. |
//! | `tx5.conn.data.recv` | `u64_observable_counter` | `By` | Bytes received on data channel. |- `remote_id`: the base64 remote peer id.<br />- `state_uniq`: endpoint identifier.<br />- `conn_uniq`: connection identifier. |
//! | `tx5.conn.data.send.message.count` | `u64_observable_counter` | | Message count sent on data channel. |- `remote_id`: the base64 remote peer id.<br />- `state_uniq`: endpoint identifier.<br />- `conn_uniq`: connection identifier. |
//! | `tx5.conn.data.recv.message.count` | `u64_observable_counter` | | Message count received on data channel. |- `remote_id`: the base64 remote peer id.<br />- `state_uniq`: endpoint identifier.<br />- `conn_uniq`: connection identifier. |
//! | `hc.conductor.p2p_event.duration`  | `f64_histogram` | `s` | The time spent processing a p2p event. |- `dna_hash`: The DNA hash that this event is being sent on behalf of. |
//! | `hc.conductor.post_commit.duration` | `f64_histogram` | `s` | The time spent executing a post commit. |- `dna_hash`: The DNA hash that this post commit is running for.<br />- `agent`: The agent running the post commit. |
//! | `hc.conductor.workflow.duration` | `f64_histogram` | `s` | The time spent running a workflow. |- `workflow`: The name of the workflow.<br />- `dna_hash`: The DNA hash that this workflow is running for.<br />- `agent`: (optional) The agent that this workflow is running for if the workflow is cell bound. |
//! | `hc.cascade.duration` | `f64_histogram` | `s` | The time taken to execute a cascade query. | |
//! | `hc.db.pool.utilization` | `f64_gauge` | | The utilisation of connections in the pool. |- `kind`: The kind of database such as Conductor, Wasm or Dht etc.<br />- `id`: The unique identifier for this database if multiple instances can exist, such as a Dht database. |
//! | `hc.db.connections.use_time` | `f64_histogram` | `s` | The time between borrowing a connection and returning it to the pool. |- `kind`: The kind of database such as Conductor, Wasm or Dht etc.<br />- `id`: The unique identifier for this database if multiple instances can exist, such as a Dht database. |
//! | `hc.ribosome.wasm.usage` | `u64_counter` | | The metered usage of a wasm ribosome. | - `dna`: The DNA hash that this wasm is metered for.<br />- `zome`: The zome that this wasm is metered for.<br />- `fn`: The function that this wasm is metered for.<br />- `agent`: The agent that this wasm is metered for (if there is one). |

#[cfg(feature = "influxive")]
const DASH_NETWORK_STATS: &[u8] = include_bytes!("dashboards/networkstats.json");
#[cfg(feature = "influxive")]
const DASH_TX5: &[u8] = include_bytes!("dashboards/tx5.json");
#[cfg(feature = "influxive")]
const DASH_DATABASE: &[u8] = include_bytes!("dashboards/database.json");
#[cfg(feature = "influxive")]
const DASH_CONDUCTOR: &[u8] = include_bytes!("dashboards/conductor.json");

/// Configuration for holochain metrics.
pub enum HolochainMetricsConfig {
    /// Metrics are disabled.
    Disabled,

    #[cfg(feature = "influxive")]
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

    #[cfg(feature = "influxive")]
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
    pub fn new(root_path: &std::path::Path) -> Self {
        #[cfg(feature = "influxive")]
        {
            const E_CHILD_SVC: &str = "HOLOCHAIN_INFLUXIVE_CHILD_SVC";

            const E_EXTERNAL: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL";
            const E_EXTERNAL_HOST: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL_HOST";
            const E_EXTERNAL_BUCKET: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL_BUCKET";
            const E_EXTERNAL_TOKEN: &str = "HOLOCHAIN_INFLUXIVE_EXTERNAL_TOKEN";

            if std::env::var_os(E_CHILD_SVC).is_some() {
                let mut database_path = std::path::PathBuf::from(root_path);
                database_path.push("influxive");
                return Self::InfluxiveChildSvc {
                    child_svc_config: Box::new(
                        influxive::InfluxiveChildSvcConfig::default()
                            .with_database_path(Some(database_path)),
                    ),
                    otel_config: influxive::InfluxiveMeterProviderConfig::default(),
                };
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
                    writer_config: influxive::InfluxiveWriterConfig::default(),
                    otel_config: influxive::InfluxiveMeterProviderConfig::default(),
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
                writer_config,
                otel_config,
                host,
                bucket,
                token,
            } => {
                Self::init_influxive_external(writer_config, otel_config, host, bucket, token);
            }
            #[cfg(feature = "influxive")]
            Self::InfluxiveChildSvc {
                child_svc_config,
                otel_config,
            } => {
                Self::init_influxive_child_svc(*child_svc_config, otel_config).await;
            }
        }
    }

    #[cfg(feature = "influxive")]
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
        opentelemetry_api::global::set_meter_provider(meter_provider);
    }

    #[cfg(feature = "influxive")]
    async fn init_influxive_child_svc(
        child_svc_config: influxive::InfluxiveChildSvcConfig,
        otel_config: influxive::InfluxiveMeterProviderConfig,
    ) {
        tracing::info!(?child_svc_config, "initializing holochain_metrics");

        match influxive::influxive_child_process_meter_provider(child_svc_config, otel_config).await
        {
            Ok((influxive, meter_provider)) => {
                // apply templates
                if let Ok(cur) = influxive.list_dashboards().await {
                    // only initialize dashboards if the db is new
                    if cur.contains("\"dashboards\": []") {
                        if let Err(err) = influxive.apply(DASH_NETWORK_STATS).await {
                            tracing::warn!(?err, "failed to initialize network stats dashboard");
                        }
                        if let Err(err) = influxive.apply(DASH_TX5).await {
                            tracing::warn!(?err, "failed to initialize tx5 dashboard");
                        }
                        if let Err(err) = influxive.apply(DASH_DATABASE).await {
                            tracing::warn!(?err, "failed to initialize database dashboard");
                        }
                        if let Err(err) = influxive.apply(DASH_CONDUCTOR).await {
                            tracing::warn!(?err, "failed to initialize conductor dashboard");
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
