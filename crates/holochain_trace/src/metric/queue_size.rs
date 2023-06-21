use opentelemetry::metrics::ObservableGauge;
use opentelemetry::sdk::metrics::data::Gauge;
use opentelemetry::{global, metrics::Histogram, Context, Key, KeyValue, StringValue, Value};
use std::time::Duration;

/// Record the number of items in a work queue.
#[derive(Clone)]
pub struct QueueSizeMetric {
    attributes: Vec<KeyValue>,
    gauge: ObservableGauge<u64>,
}

impl QueueSizeMetric {
    /// Create a new instance for a named queue.
    pub fn new<T>(queue_name: T) -> Self
    where
        T: Into<StringValue>,
    {
        let meter = global::meter("holochain.queue_size");
        let gauge = meter.u64_observable_gauge("queue_size").init();

        QueueSizeMetric {
            attributes: vec![KeyValue {
                key: Key::from_static_str("queue_name"),
                value: Value::String(queue_name.into()),
            }],
            gauge,
        }
    }

    /// Record the current queue size.
    pub fn record_size(&self, size: u64) {
        self.gauge.observe(size, &self.attributes);
    }
}
