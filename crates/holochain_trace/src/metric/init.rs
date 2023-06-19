use opentelemetry::global;
use opentelemetry::metrics::MetricsError;
use opentelemetry::sdk::metrics::{MeterProvider, PeriodicReader};
use opentelemetry::sdk::runtime;
use opentelemetry_otlp::{ExportConfig, WithExportConfig};
use std::time::Duration;

/// Initialise metrics reporting
pub fn init_metrics() -> Result<(), MetricsError> {
    match std::env::var("OTEL_EXPORT") {
        Ok(value) => match value.as_str() {
            "otlp" => {
                configure_otlp_exporter()?;
            }
            _ => {
                configure_stdout_exporter();
            }
        },
        Err(e) => match e {
            std::env::VarError::NotPresent => {
                configure_stdout_exporter();
            }
            e => {
                tracing::warn!("Could not configure metrics exporter {:?}", e);
            }
        },
    }

    Ok(())
}

fn configure_stdout_exporter() {
    let exporter = opentelemetry_stdout::MetricsExporter::default();
    let reader = PeriodicReader::builder(exporter, runtime::Tokio)
        .with_interval(Duration::from_secs(15))
        .build();
    let provider = MeterProvider::builder().with_reader(reader).build();
    global::set_meter_provider(provider);
}

fn configure_otlp_exporter() -> Result<(), MetricsError> {
    let export_config = ExportConfig {
        endpoint: "http://localhost:4317".to_string(),
        ..ExportConfig::default()
    };
    let meter = opentelemetry_otlp::new_pipeline()
        .metrics(runtime::Tokio)
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_export_config(export_config),
        )
        .build()?;

    global::set_meter_provider(meter);

    Ok(())
}
