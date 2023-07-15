/// Initialize metrics. Call this from binaries, not libraries.
pub async fn initialize_metrics(path: std::path::PathBuf) {
    tracing::warn!(?path, "RUN INFLUXIVE");
    match influxive_child_svc::Influxive::new(influxive_child_svc::Config {
        database_path: Some(path),
        ..Default::default()
    })
    .await
    {
        Ok(influxive) => {
            opentelemetry_api::global::set_meter_provider(
                influxive_otel::InfluxiveMeterProvider::new(std::sync::Arc::new(influxive)),
            );
        }
        Err(err) => {
            tracing::warn!(?err, "unable to initialize local metrics");
        }
    }
}
