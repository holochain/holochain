use opentelemetry::{global, metrics::Histogram, Context, Key, KeyValue, StringValue, Value};
use std::time::Duration;

/// Record run cycles for a Tokio task.
#[derive(Clone)]
pub struct RequestResponseDurationMetric {
    attributes: Vec<KeyValue>,
    hist: Histogram<u64>,
}

impl RequestResponseDurationMetric {
    /// Create a new metric handle with a unique name for the handler.
    pub fn new<T>(handler_name: T) -> Self
    where
        T: Into<StringValue>,
    {
        let meter = global::meter("holochain.request_response");
        let histogram = meter.u64_histogram("request_response_duration").init();

        RequestResponseDurationMetric {
            attributes: vec![KeyValue {
                key: Key::from_static_str("handler_name"),
                value: Value::String(handler_name.into()),
            }],
            hist: histogram,
        }
    }

    /// Record the duration of a single request/response.
    pub fn record_duration(&self, duration: Duration) {
        let ctx = Context::current();
        self.hist
            .record(&ctx, duration.as_millis() as u64, &self.attributes);
    }
}
