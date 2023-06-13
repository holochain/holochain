use opentelemetry::{global, metrics::Counter, Context, Key, KeyValue, Value};

/// Record run cycles for a Tokio task.
pub struct TaskRunMetric {
    attributes: Vec<KeyValue>,
    counter: Counter<u64>,
}

impl TaskRunMetric {
    /// Create a new metric handle with a unique name for the current task.
    pub fn new(task_name: &'static str) -> Self {
        let meter = global::meter("holochain.task");
        let counter = meter.u64_counter("run_count").init();

        TaskRunMetric {
            attributes: vec![KeyValue {
                key: Key::from_static_str("task_name"),
                value: Value::String(task_name.into()),
            }],
            counter,
        }
    }

    /// Record a task cycle starting.
    pub fn record_start(&self) {
        let ctx = Context::current();
        self.counter.add(&ctx, 1, &self.attributes);
    }
}
