#![deny(missing_docs)]
#![deny(warnings)]
#![deny(unsafe_code)]
//! High-level Rust integration of opentelemetry metrics and InfluxDB.
//!
//! ## Usage
//!
//! 1. Create a meter provider using one of the provided functions.
//! 2. Register it globally via `opentelemetry::global::set_meter_provider`.
//! 3. Obtain meters from the global provider with `opentelemetry::global::meter`.

use std::sync::Arc;

mod child_svc;
mod otel;
mod types;
mod writer;

pub(crate) use child_svc::*;
pub(crate) use otel::*;
pub(crate) use types::InfluxiveResult;
pub(crate) use writer::{InfluxiveWriter, InfluxiveWriterConfig};

/// Create an opentelemetry MeterProvider ready to provide metrics
/// to a running child process instance of InfluxDB.
pub(crate) async fn influxive_child_process_meter_provider(
    svc_config: InfluxiveChildSvcConfig,
    otel_config: InfluxiveMeterProviderConfig,
) -> InfluxiveResult<(Arc<InfluxiveChildSvc>, InfluxiveMeterProvider)> {
    let influxive = Arc::new(InfluxiveChildSvc::new(svc_config).await?);
    let meter_provider = InfluxiveMeterProvider::new(otel_config, influxive.clone());
    Ok((influxive, meter_provider))
}

/// Create an opentelemetry MeterProvider ready to provide metrics
/// to an InfluxDB instance that is already running as a separate process.
pub(crate) fn influxive_external_meter_provider_token_auth<
    H: AsRef<str>,
    B: AsRef<str>,
    T: AsRef<str>,
>(
    writer_config: InfluxiveWriterConfig,
    otel_config: InfluxiveMeterProviderConfig,
    host: H,
    bucket: B,
    token: T,
) -> InfluxiveMeterProvider {
    let writer = InfluxiveWriter::with_token_auth(writer_config, host, bucket, token);
    InfluxiveMeterProvider::new(otel_config, Arc::new(writer))
}

/// Create an opentelemetry MeterProvider ready to provide metrics
/// to a file on disk.
pub(crate) fn influxive_file_meter_provider(
    writer_config: InfluxiveWriterConfig,
    otel_config: InfluxiveMeterProviderConfig,
) -> InfluxiveMeterProvider {
    // host/bucket/token are not needed when using a file writer
    let writer = InfluxiveWriter::with_token_auth(writer_config, "", "", "");
    InfluxiveMeterProvider::new(otel_config, Arc::new(writer))
}

#[cfg(test)]
mod tests;
