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

use crate::types::DynMetricWriter;
use opentelemetry::metrics::{Meter, MeterProvider};
use opentelemetry::InstrumentationScope;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, Metric, MetricData, ResourceMetrics};
use opentelemetry_sdk::metrics::exporter::PushMetricExporter;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider, Temporality};
use std::sync::Arc;
use std::time::SystemTime;

/// The writer
pub struct InfluxiveOtelWriter {
    influxive: DynMetricWriter,
}

impl InfluxiveOtelWriter {
    fn write(&self, otel_metric: &Metric) {
        match otel_metric.data() {
            AggregatedMetrics::F64(metric_data) => match metric_data {
                MetricData::Histogram(histogram) => {
                    for data_point in histogram.data_points() {
                        let mut influxive_metric =
                            crate::types::Metric::new(SystemTime::now(), otel_metric.name())
                                .with_field("count", data_point.count())
                                .with_field("sum", data_point.sum());
                        if let Some(min) = data_point.min() {
                            influxive_metric = influxive_metric.with_field("min", min);
                        }
                        if let Some(max) = data_point.max() {
                            influxive_metric = influxive_metric.with_field("max", max);
                        }
                        self.influxive.write_metric(influxive_metric);
                    }
                }
                MetricData::Gauge(gauge) => {
                    for data_point in gauge.data_points() {
                        let mut influxive_metric =
                            crate::types::Metric::new(SystemTime::now(), otel_metric.name())
                                .with_field("gauge", data_point.value());
                        for attribute in data_point.attributes() {
                            influxive_metric = influxive_metric
                                .with_tag(attribute.key.to_string(), attribute.value.to_string());
                        }
                        self.influxive.write_metric(influxive_metric);
                    }
                }
                _ => unimplemented!(),
            },
            AggregatedMetrics::U64(metric_data) => match metric_data {
                MetricData::Sum(sum) => {
                    for data_point in sum.data_points() {
                        let mut influxive_metric =
                            crate::types::Metric::new(SystemTime::now(), otel_metric.name())
                                .with_field("sum", data_point.value());
                        for attribute in data_point.attributes() {
                            influxive_metric = influxive_metric
                                .with_tag(attribute.key.to_string(), attribute.value.to_string());
                        }
                        self.influxive.write_metric(influxive_metric);
                    }
                }
                _ => unimplemented!(),
            },
            _ => unimplemented!(),
        }
    }
}

impl PushMetricExporter for InfluxiveOtelWriter {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        for scope_metrics in metrics.scope_metrics() {
            for metric in scope_metrics.metrics() {
                self.write(metric);
            }
        }
        Ok(())
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: std::time::Duration) -> OTelSdkResult {
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
    pub fn with_report_interval(
        mut self,
        observable_report_interval: Option<std::time::Duration>,
    ) -> Self {
        self.observable_report_interval = observable_report_interval;
        self
    }
}

/// Meter provider to create meters for collecting metrics and writing them to
/// and Influx DB.
#[derive(Clone)]
pub struct InfluxiveMeterProvider {
    inner: Arc<SdkMeterProvider>,
}

impl InfluxiveMeterProvider {
    /// Construct a new InfluxiveMeterProvider instance with a given
    /// "Influxive" InfluxiveDB child process connector.
    pub fn new(config: InfluxiveMeterProviderConfig, influxive: DynMetricWriter) -> Self {
        let exporter = InfluxiveOtelWriter { influxive };
        let mut reader_builder = PeriodicReader::builder(exporter);
        if let Some(interval) = config.observable_report_interval {
            reader_builder = reader_builder.with_interval(interval);
        }
        let reader = reader_builder.build();
        let provider = SdkMeterProvider::builder().with_reader(reader).build();
        Self {
            inner: Arc::new(provider),
        }
    }
}

impl MeterProvider for InfluxiveMeterProvider {
    fn meter(&self, name: &'static str) -> Meter {
        self.inner.meter(name)
    }

    fn meter_with_scope(&self, _scope: InstrumentationScope) -> Meter {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests;
