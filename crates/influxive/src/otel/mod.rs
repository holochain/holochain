#![deny(missing_docs)]
// #![deny(warnings)]
#![deny(unsafe_code)]
//! Opentelemetry metrics bindings for influxive-child-svc.
//!
//! ## Example
//!
//! ```
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() {
//! #     use std::sync::Arc;
//! use influxive::writer::*;
//!
//! // create an influxive writer
//! let writer = InfluxiveWriter::with_token_auth(
//!     InfluxiveWriterConfig::default(),
//!     "http://127.0.0.1:8086",
//!     "my.bucket",
//!     "my.token",
//! );
//!
//! // register the meter provider
//! opentelemetry_api::global::set_meter_provider(
//!     influxive::otel::InfluxiveMeterProvider::new(
//!         Default::default(),
//!         Arc::new(writer),
//!     )
//! );
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

use opentelemetry::metrics::{InstrumentProvider, Meter, MeterProvider};
use opentelemetry::InstrumentationScope;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::exporter::PushMetricExporter;
use opentelemetry_sdk::metrics::{
    PeriodicReader, PeriodicReaderBuilder, SdkMeterProvider, Temporality,
};
use std::sync::Arc;
use std::time::Duration;

/// The writer
pub struct InfluxiveWriter {
    client: influxdb::Client,
}

impl InfluxiveWriter {
    fn write(&self) {
        println!("writing");
        // self.client.
    }
}

impl PushMetricExporter for InfluxiveWriter {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        println!("export called with metrics {:?}", metrics);
        self.write();
        Ok(())
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, timeout: std::time::Duration) -> OTelSdkResult {
        Ok(())
    }

    fn temporality(&self) -> Temporality {
        Temporality::Cumulative
    }
}

/// Influxive InfluxDB Meter Provider Configuration.
#[non_exhaustive]
pub struct InfluxiveMeterProviderConfig {
    /// Reporting interval for observable metrics.
    /// Set to `None` to disable periodic reporting
    /// (you'll need to call [InfluxiveMeterProvider::report] manually).
    /// Defaults to 30 seconds.
    pub observable_report_interval: Option<std::time::Duration>,
}

impl Default for InfluxiveMeterProviderConfig {
    fn default() -> Self {
        Self {
            observable_report_interval: Some(std::time::Duration::from_secs(30)),
        }
    }
}

impl InfluxiveMeterProviderConfig {
    /// Apply [InfluxiveMeterProviderConfig::observable_report_interval].
    pub fn with_observable_report_interval(
        mut self,
        observable_report_interval: Option<std::time::Duration>,
    ) -> Self {
        self.observable_report_interval = observable_report_interval;
        self
    }
}

#[derive(Clone)]
pub(crate) struct InfluxiveMeterProvider {
    inner: Arc<SdkMeterProvider>,
}

impl InfluxiveMeterProvider {
    /// Construct a new InfluxiveMeterProvider instance with a given
    /// "Influxive" InfluxiveDB child process connector.
    pub fn new(config: InfluxiveMeterProviderConfig, client: influxdb::Client) -> Self {
        let exporter = InfluxiveWriter { client };
        let reader = PeriodicReader::builder(exporter)
            .with_interval(Duration::from_millis(10))
            .build();
        let mut provider = SdkMeterProvider::builder().with_reader(reader);
        if let Some(interval) = config.observable_report_interval {}

        let provider = provider.build();
        Self {
            inner: Arc::new(provider),
        }
    }
}

impl MeterProvider for InfluxiveMeterProvider {
    fn meter(&self, name: &'static str) -> Meter {
        self.inner.meter(name)
    }

    fn meter_with_scope(&self, scope: InstrumentationScope) -> Meter {
        todo!()
    }
}

#[cfg(test)]
mod tests;
