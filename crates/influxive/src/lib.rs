#![deny(missing_docs)]
#![deny(warnings)]
#![deny(unsafe_code)]
//! High-level Rust integration of opentelemetry metrics and InfluxDB.
//!
//! ## Examples
//!
//! ### Easy, zero-configuration InfluxDB as a child process
//!
//! ```
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() {
//! let tmp = tempfile::tempdir().unwrap();
//!
//! // create our meter provider
//! let (_influxive, meter_provider) = influxive::influxive_child_process_meter_provider(
//!     influxive::InfluxiveChildSvcConfig::default()
//!         .with_database_path(Some(tmp.path().to_owned())),
//!     influxive::InfluxiveMeterProviderConfig::default(),
//! ).await.unwrap();
//!
//! // register our meter provider
//! opentelemetry_api::global::set_meter_provider(meter_provider);
//!
//! // create a metric
//! let m = opentelemetry_api::global::meter("my.meter")
//!     .f64_histogram("my.metric")
//!     .init();
//!
//! // make a recording
//! m.record(3.14, &[]);
//! # _influxive.shutdown();
//! # }
//! ```
//!
//! ### Connecting to an already running InfluxDB system process
//!
//! ```
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() {
//! // create our meter provider
//! let meter_provider = influxive::influxive_external_meter_provider_token_auth(
//!     influxive::InfluxiveWriterConfig::default(),
//!     influxive::InfluxiveMeterProviderConfig::default(),
//!     "http://127.0.0.1:8086",
//!     "my.bucket",
//!     "my.token",
//! );
//!
//! // register our meter provider
//! opentelemetry_api::global::set_meter_provider(meter_provider);
//!
//! // create a metric
//! let m = opentelemetry_api::global::meter("my.meter")
//!     .f64_histogram("my.metric")
//!     .init();
//!
//! // make a recording
//! m.record(3.14, &[]);
//! # }
//! ```
//!
//! ### Writing to an influx file
//!
//! ```
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() {
//! // create our meter provider
//! let meter_provider = influxive::influxive_file_meter_provider(
//!     influxive::InfluxiveWriterConfig::create_with_influx_file(std::path::PathBuf::from("my-metrics.influx")),
//!     influxive::InfluxiveMeterProviderConfig::default(),
//! );
//!
//! // register our meter provider
//! opentelemetry_api::global::set_meter_provider(meter_provider);
//!
//! // create a metric
//! let m = opentelemetry_api::global::meter("my.meter")
//!     .f64_histogram("my.metric")
//!     .init();
//!
//! // make a recording
//! m.record(3.14, &[]);
//!
//! // Read and use data in "my-metrics.influx"
//!
//! # std::fs::remove_file("my-metrics.influx").unwrap();
//! # }
//! ```

use std::sync::Arc;

use child_svc::InfluxiveChildSvc;
use otel::InfluxiveMeterProvider;
use writer::*;

#[doc(inline)]
pub use child_svc::InfluxiveChildSvcConfig;

#[doc(inline)]
pub use writer::InfluxiveWriterConfig;

#[doc(inline)]
pub use otel::InfluxiveMeterProviderConfig;

pub mod child_svc;
mod downloader;
pub mod otel;
pub mod types;
pub mod writer;

/// Create an opentelemetry_api MeterProvider ready to provide metrics
/// to a running child process instance of InfluxDB.
pub async fn influxive_child_process_meter_provider(
    svc_config: InfluxiveChildSvcConfig,
    otel_config: InfluxiveMeterProviderConfig,
) -> std::io::Result<(Arc<InfluxiveChildSvc>, InfluxiveMeterProvider)> {
    let influxive = Arc::new(InfluxiveChildSvc::new(svc_config).await?);
    let meter_provider = InfluxiveMeterProvider::new(otel_config, influxive.clone());
    Ok((influxive, meter_provider))
}

/// Create an opentelemetry_api MeterProvider ready to provide metrics
/// to an InfluxDB instance that is already running as a separate process.
pub fn influxive_external_meter_provider_token_auth<H: AsRef<str>, B: AsRef<str>, T: AsRef<str>>(
    writer_config: InfluxiveWriterConfig,
    otel_config: InfluxiveMeterProviderConfig,
    host: H,
    bucket: B,
    token: T,
) -> InfluxiveMeterProvider {
    let writer = InfluxiveWriter::with_token_auth(writer_config, host, bucket, token);
    InfluxiveMeterProvider::new(otel_config, Arc::new(writer))
}

/// Create an opentelemetry_api MeterProvider ready to provide metrics
/// to a file on disk.
pub fn influxive_file_meter_provider(
    writer_config: InfluxiveWriterConfig,
    otel_config: InfluxiveMeterProviderConfig,
) -> InfluxiveMeterProvider {
    // host/bucket/token are not needed when using a file writer
    let writer = InfluxiveWriter::with_token_auth(writer_config, "", "", "");
    InfluxiveMeterProvider::new(otel_config, Arc::new(writer))
}

#[cfg(test)]
mod test;
