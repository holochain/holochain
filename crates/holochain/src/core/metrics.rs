use holo_hash::{AgentPubKey, DnaHash};
use opentelemetry::{global::meter, metrics::Histogram, KeyValue};
use std::sync::Arc;

pub struct WorkflowDurationMetric {
    histogram: Histogram<f64>,
    attributes: Vec<KeyValue>,
}

impl WorkflowDurationMetric {
    pub fn record(&self, value: f64) {
        self.histogram.record(value, &self.attributes);
    }
}

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

    WorkflowDurationMetric {
        histogram,
        attributes,
    }
}
