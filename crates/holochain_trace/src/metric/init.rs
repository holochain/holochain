use opentelemetry::global;
use opentelemetry::sdk::metrics::{MeterProvider, PeriodicReader};
use opentelemetry::sdk::runtime;
use std::time::Duration;

/// Initialise metrics reporting
pub fn init_metrics() {
    let exporter = opentelemetry_stdout::MetricsExporter::default();
    let reader = PeriodicReader::builder(exporter, runtime::Tokio)
        .with_interval(Duration::from_secs(15))
        .build();
    let provider = MeterProvider::builder().with_reader(reader).build();
    global::set_meter_provider(provider);
}
