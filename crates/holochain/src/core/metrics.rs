use std::sync::Arc;

use holo_hash::{AgentPubKey, DnaHash};
use opentelemetry_api::{global::meter_with_version, metrics::*, KeyValue};

pub type WorkflowDurationMetric = Histogram<f64>;

pub fn create_workflow_duration_metric(
    workflow_name: String,
    dna_hash: Arc<DnaHash>,
    agent: Option<AgentPubKey>,
) -> WorkflowDurationMetric {
    let mut attr = vec![
        KeyValue::new("workflow", workflow_name),
        KeyValue::new("dna_hash", format!("{:?}", dna_hash)),
    ];

    if let Some(agent) = agent {
        attr.push(KeyValue::new("agent", format!("{:?}", agent)));
    }

    meter_with_version(
        "hc.conductor",
        None::<&'static str>,
        None::<&'static str>,
        Some(attr),
    )
    .f64_histogram("hc.conductor.workflow.duration")
    .with_unit(Unit::new("s"))
    .with_description("The time spent running a workflow")
    .init()
}
