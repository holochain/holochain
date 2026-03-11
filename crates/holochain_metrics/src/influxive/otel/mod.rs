#![deny(missing_docs)]
#![deny(unsafe_code)]
//! OpenTelemetry metrics exporter for Influxive.

use super::types::DynMetricWriter;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, Metric, MetricData, ResourceMetrics};
use opentelemetry_sdk::metrics::exporter::PushMetricExporter;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider, Temporality};

#[cfg(test)]
mod tests;

/// The writer
pub struct InfluxiveOtelWriter {
    influxive: DynMetricWriter,
}

impl InfluxiveOtelWriter {
    fn write(&self, otel_metric: &Metric) {
        match otel_metric.data() {
            AggregatedMetrics::F64(metric_data) => match metric_data {
                MetricData::Histogram(histogram) => {
                    self.write_histogram(histogram, otel_metric.name());
                }
                MetricData::Gauge(gauge) => {
                    for data_point in gauge.data_points() {
                        let mut influxive_metric =
                            super::types::Metric::new(gauge.time(), otel_metric.name())
                                .with_field("gauge", data_point.value());
                        for attribute in data_point.attributes() {
                            influxive_metric = influxive_metric
                                .with_tag(attribute.key.to_string(), attribute.value.to_string());
                        }
                        self.influxive.write_metric(influxive_metric);
                    }
                }
                unimplemented_metric => {
                    tracing::error!(?unimplemented_metric, "metric not implemented")
                }
            },
            AggregatedMetrics::I64(MetricData::Sum(sum)) => {
                self.write_sum(sum, otel_metric.name());
            }
            AggregatedMetrics::U64(MetricData::Sum(sum)) => {
                self.write_sum(sum, otel_metric.name());
            }
            AggregatedMetrics::U64(MetricData::Histogram(histogram)) => {
                self.write_histogram(histogram, otel_metric.name());
            }
            unimplemented_metric => {
                tracing::error!(?unimplemented_metric, "metric not implemented")
            }
        }
    }

    fn write_sum<T>(&self, sum: &opentelemetry_sdk::metrics::data::Sum<T>, name: &str)
    where
        T: Copy + Into<super::types::DataType>,
    {
        for data_point in sum.data_points() {
            let mut influxive_metric =
                super::types::Metric::new(sum.time(), name).with_field("sum", data_point.value());
            for attribute in data_point.attributes() {
                influxive_metric = influxive_metric
                    .with_tag(attribute.key.to_string(), attribute.value.to_string());
            }
            self.influxive.write_metric(influxive_metric);
        }
    }

    fn write_histogram<T>(
        &self,
        histogram: &opentelemetry_sdk::metrics::data::Histogram<T>,
        name: &str,
    ) where
        T: Copy + Into<super::types::DataType>,
    {
        for data_point in histogram.data_points() {
            let mut influxive_metric = super::types::Metric::new(histogram.time(), name)
                .with_field("count", data_point.count())
                .with_field("sum", data_point.sum());
            if let Some(min) = data_point.min() {
                influxive_metric = influxive_metric.with_field("min", min);
            }
            if let Some(max) = data_point.max() {
                influxive_metric = influxive_metric.with_field("max", max);
            }
            for attribute in data_point.attributes() {
                influxive_metric = influxive_metric
                    .with_tag(attribute.key.to_string(), attribute.value.to_string());
            }
            self.influxive.write_metric(influxive_metric);
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
    /// Reporting interval for all metrics, sync and observable.
    ///
    /// The interval can also be configured with the env var OTEL_METRIC_EXPORT_INTERVAL.
    /// This option has to be set to `None` for the env var to be effective, otherwise it
    /// overrides any value set for the OTEL_METRIC_EXPORT_INTERVAL environment variable.
    ///
    /// If this option is `None` or the interval is equal to zero, 10 seconds is used as the default.
    ///
    /// Defaults to None, which results in a 10 second interval.
    pub report_interval: Option<std::time::Duration>,
}

impl Default for InfluxiveMeterProviderConfig {
    fn default() -> Self {
        let report_interval = if let Ok(interval) = std::env::var("OTEL_METRIC_EXPORT_INTERVAL") {
            // If env var is incorrect, panic.
            Some(std::time::Duration::from_millis(interval.parse().expect(
                "OTEL_METRIC_EXPORT_INTERVAL is not set to a valid integer",
            )))
        } else {
            // If env var isn't set, default to 10 seconds.
            Some(std::time::Duration::from_secs(10))
        };
        Self { report_interval }
    }
}

impl InfluxiveMeterProviderConfig {
    /// Apply [`InfluxiveMeterProviderConfig::report_interval`].
    pub fn with_report_interval(mut self, report_interval: Option<std::time::Duration>) -> Self {
        self.report_interval = report_interval;
        self
    }
}

/// Create a meter provider to create meters for collecting metrics and writing
/// them to an Influx DB with a given Influxive writer.
pub fn create_meter_provider(
    config: InfluxiveMeterProviderConfig,
    influxive: DynMetricWriter,
) -> SdkMeterProvider {
    let exporter = InfluxiveOtelWriter { influxive };
    let mut reader_builder = PeriodicReader::builder(exporter);
    if let Some(interval) = config.report_interval {
        reader_builder = reader_builder.with_interval(interval);
    }
    let reader = reader_builder.build();
    SdkMeterProvider::builder().with_reader(reader).build()
}
