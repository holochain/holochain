const DASH_NETWORK_STATS: &[u8] = include_bytes!("dashboards/networkstats.json");

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
            // apply templates
            if let Ok(cur) = influxive.list_dashboards().await {
                // only initialize dashboards if the db is new
                if cur.contains("\"dashboards\": []") {
                    let _ = influxive.apply(DASH_NETWORK_STATS).await;
                }
            }
            if let Ok(cur) = influxive.list_dashboards().await {
                // only initialize dashboards if the db is new
                if cur.contains("\"dashboards\": []") {
                    let _ = influxive.apply(DASH_NETWORK_STATS).await;
                }
            }

            opentelemetry_api::global::set_meter_provider(
                influxive_otel::InfluxiveMeterProvider::new(std::sync::Arc::new(influxive)),
            );
        }
        Err(err) => {
            tracing::warn!(?err, "unable to initialize local metrics");
        }
    }
}
