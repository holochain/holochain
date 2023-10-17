use super::DbKind;

pub type PoolUsageMetric = opentelemetry_api::metrics::UpDownCounter<i64>;
pub type UseTimeMetric = opentelemetry_api::metrics::Histogram<u64>;

pub fn create_pool_usage_metric(kind: DbKind) -> PoolUsageMetric {
    opentelemetry_api::global::meter_with_version(
        "hc.db.connections",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![
            opentelemetry_api::KeyValue::new("state", "idle"),
            opentelemetry_api::KeyValue::new("kind", format!("{}", kind)),
        ]),
    )
    .i64_up_down_counter("connections")
    .with_unit(opentelemetry_api::metrics::Unit::new("connections"))
    .with_description("The number of idle connections in the pool")
    .init()
}

pub fn create_connection_use_time_metric(kind: DbKind) -> UseTimeMetric {
    opentelemetry_api::global::meter_with_version(
        "hc.db.connections.use_time",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![opentelemetry_api::KeyValue::new(
            "kind",
            format!("{}", kind),
        )]),
    )
    .u64_histogram("use_time")
    .with_unit(opentelemetry_api::metrics::Unit::new("ms"))
    .with_description("	The time between borrowing a connection and returning it to the pool")
    .init()
}
