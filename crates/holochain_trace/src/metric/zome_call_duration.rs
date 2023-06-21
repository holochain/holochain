use opentelemetry::{global, metrics::Histogram, Context, Key, KeyValue, StringValue, Value};
use std::time::Duration;

/// Record zome call durations.
pub struct ZomeCallDurationMetric {
    hist: Histogram<u64>,
}

impl ZomeCallDurationMetric {
    /// Create a new `ZomeCallDurationMetric`.
    pub fn new() -> Self {
        let meter = global::meter("holochain.zome_call_duration");
        let histogram = meter.u64_histogram("zome_call_duration").init();

        ZomeCallDurationMetric { hist: histogram }
    }

    /// Record the duration of a single call.
    pub fn record_duration<T>(&self, func_name: String, duration: Duration)
    where
        T: Into<StringValue>,
    {
        let ctx = Context::current();

        let attributes = vec![KeyValue {
            key: Key::from_static_str("func_name"),
            value: Value::String(func_name.into()),
        }];

        self.hist
            .record(&ctx, duration.as_millis() as u64, &attributes);
    }
}
