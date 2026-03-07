use holo_hash::{AgentPubKey, DnaHash};
use opentelemetry::{global::meter, metrics, KeyValue};
use std::sync::Arc;

pub struct Histogram {
    histogram: metrics::Histogram<f64>,
    attributes: Vec<KeyValue>,
}

impl Histogram {
    pub fn record(&self, value: f64) {
        self.histogram.record(value, &self.attributes);
    }
}

pub type WorkflowDurationMetric = Histogram;

pub fn create_workflow_duration_metric(
    workflow_name: String,
    dna_hash: Arc<DnaHash>,
    agent: Option<AgentPubKey>,
) -> WorkflowDurationMetric {
    let mut attributes = vec![
        KeyValue::new("workflow", workflow_name),
        KeyValue::new("dna_hash", format!("{dna_hash:?}")),
    ];

    if let Some(agent) = agent {
        attributes.push(KeyValue::new("agent", format!("{agent:?}")));
    }

    let histogram = meter("hc.conductor")
        .f64_histogram("hc.conductor.workflow.duration")
        .with_unit("s")
        .with_description("The time spent running a workflow")
        .build();

    Histogram {
        histogram,
        attributes,
    }
}

pub type WasmUsageMetric = metrics::Counter<u64>;

pub fn create_ribosome_wasm_usage_metric() -> WasmUsageMetric {
    meter("hc.ribosome.wasm")
        .u64_counter("hc.ribosome.wasm.usage")
        .with_description("The metered usage of a wasm ribosome.")
        .build()
}

pub type WasmCallDurationMetric = metrics::Histogram<f64>;
pub fn create_ribosome_wasm_call_duration_metric() -> WasmCallDurationMetric {
    meter("hc.ribosome.wasm")
        .f64_histogram("hc.ribosome.wasm_call.duration")
        .with_unit("s")
        .with_description("The time spent running a wasm call.")
        .build()
}
