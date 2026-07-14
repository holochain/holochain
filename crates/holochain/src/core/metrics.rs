use holo_hash::DnaHash;
use opentelemetry::{global::meter, metrics, KeyValue};
use std::sync::{Arc, OnceLock};

pub(crate) struct Histogram {
    histogram: metrics::Histogram<f64>,
    attributes: Vec<KeyValue>,
}

impl Histogram {
    pub fn record(&self, value: f64) {
        self.histogram.record(value, &self.attributes);
    }
}

pub(crate) type WorkflowDurationMetric = Histogram;

pub(crate) fn create_workflow_duration_metric(
    workflow_name: String,
    dna_hash: Arc<DnaHash>,
) -> WorkflowDurationMetric {
    let attributes = vec![
        KeyValue::new("workflow", workflow_name),
        KeyValue::new("dna_hash", dna_hash.to_string()),
    ];

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

pub(crate) type IntegratedOpMetric = metrics::Counter<u64>;

static INTEGRATED_OP_METRIC: OnceLock<IntegratedOpMetric> = OnceLock::new();

pub(crate) fn workflow_integrated_op_metric() -> &'static IntegratedOpMetric {
    INTEGRATED_OP_METRIC.get_or_init(|| {
        meter("hc.conductor")
            .u64_counter("hc.conductor.workflow.integrated_ops")
            .with_description("The number of integrated operations.")
            .build()
    })
}

pub(crate) type OpIntegrationDelayMetric = metrics::Histogram<f64>;

static OP_INTEGRATION_DELAY_METRIC: OnceLock<OpIntegrationDelayMetric> = OnceLock::new();

pub(crate) fn op_integration_delay_metric() -> &'static OpIntegrationDelayMetric {
    OP_INTEGRATION_DELAY_METRIC.get_or_init(|| {
        meter("hc.conductor")
            .f64_histogram("hc.conductor.workflow.integration_delay")
            .with_unit("s")
            .with_description("Time between an op being stored and it being integrated.")
            .build()
    })
}

pub(crate) type OpValidationAttemptsMetric = metrics::Histogram<u64>;

static OP_VALIDATION_ATTEMPTS_METRIC: OnceLock<OpValidationAttemptsMetric> = OnceLock::new();

pub(crate) fn op_validation_attempts_metric() -> &'static OpValidationAttemptsMetric {
    OP_VALIDATION_ATTEMPTS_METRIC.get_or_init(|| {
        meter("hc.conductor")
            .u64_histogram("hc.conductor.workflow.validation_attempts")
            .with_description("Number of validation attempts required to integrate an op.")
            .build()
    })
}

/// Record integration metrics once per source-chain flush that integrated
/// locally-authored ops.
///
/// Locally-authored ops are integrated immediately at source
/// chain flush, bypassing the integration workflow's `summaries` path that
/// records [`op_integration_delay_metric`] / [`op_validation_attempts_metric`]
/// for network-received ops. Authored ops have no integration delay and need no
/// validation retries, so both samples are always zero — a single zeroed sample
/// per flush is therefore enough to keep these histograms exporting for
/// authored data, without recomputing the per-op count here. Per-op emission is
/// left to the future shared inline-integration helper that will own the
/// integrate/publish/metrics step for authored data.
pub(crate) fn record_authored_op_integration(dna_hash: &DnaHash) {
    let attributes = [KeyValue::new("dna_hash", dna_hash.to_string())];
    op_integration_delay_metric().record(0.0, &attributes);
    op_validation_attempts_metric().record(0, &attributes);
}

#[cfg(any(feature = "wasmer-sys-cranelift", feature = "wasmer-sys-llvm"))]
pub(crate) type WasmUsageMetric = metrics::Counter<u64>;

#[cfg(any(feature = "wasmer-sys-cranelift", feature = "wasmer-sys-llvm"))]
static WASM_USAGE_METRIC: OnceLock<WasmUsageMetric> = OnceLock::new();

#[cfg(any(feature = "wasmer-sys-cranelift", feature = "wasmer-sys-llvm"))]
pub(crate) fn ribosome_wasm_usage_metric() -> &'static WasmUsageMetric {
    WASM_USAGE_METRIC.get_or_init(|| {
        meter("hc.ribosome.wasm")
            .u64_counter("hc.ribosome.wasm.usage")
            .with_description("The metered usage of a wasm ribosome.")
            .build()
    })
}

pub(crate) type WasmCallDurationMetric = metrics::Histogram<f64>;

static WASM_CALL_DURATION_METRIC: OnceLock<WasmCallDurationMetric> = OnceLock::new();

pub(crate) fn ribosome_wasm_call_duration_metric() -> &'static WasmCallDurationMetric {
    WASM_CALL_DURATION_METRIC.get_or_init(|| {
        meter("hc.ribosome.wasm")
            .f64_histogram("hc.ribosome.wasm_call.duration")
            .with_unit("s")
            .with_description("The time spent running a wasm call.")
            .build()
    })
}

pub(crate) type ZomeCallDurationMetric = metrics::Histogram<f64>;

static ZOME_CALL_DURATION_METRIC: OnceLock<ZomeCallDurationMetric> = OnceLock::new();

pub(crate) fn ribosome_zome_call_duration_metric() -> &'static ZomeCallDurationMetric {
    ZOME_CALL_DURATION_METRIC.get_or_init(|| {
        meter("hc.ribosome.wasm")
            .f64_histogram("hc.ribosome.zome_call.duration")
            .with_unit("s")
            .with_description("The time spent running a zome call.")
            .build()
    })
}

pub(crate) type HostFnCallDurationMetric = metrics::Histogram<f64>;

static HOST_FN_CALL_DURATION_METRIC: OnceLock<HostFnCallDurationMetric> = OnceLock::new();

pub(crate) fn host_fn_call_duration_metric() -> &'static HostFnCallDurationMetric {
    HOST_FN_CALL_DURATION_METRIC.get_or_init(|| {
        meter("hc.ribosome")
            .f64_histogram("hc.ribosome.host_fn_call.duration")
            .with_unit("s")
            .with_description("The time spent executing a host function call.")
            .build()
    })
}

pub(crate) type EmitSignalMetric = metrics::Counter<u64>;

static EMIT_SIGNAL_METRIC: OnceLock<EmitSignalMetric> = OnceLock::new();

pub(crate) fn emit_signal_metric() -> &'static EmitSignalMetric {
    EMIT_SIGNAL_METRIC.get_or_init(|| {
        meter("hc.ribosome")
            .u64_counter("hc.ribosome.host_fn.emit_signal.count")
            .with_description("The number of local signals emitted.")
            .build()
    })
}

pub(crate) type SendRemoteSignalMetric = metrics::Counter<u64>;

static SEND_REMOTE_SIGNAL_METRIC: OnceLock<SendRemoteSignalMetric> = OnceLock::new();

pub(crate) fn send_remote_signal_metric() -> &'static SendRemoteSignalMetric {
    SEND_REMOTE_SIGNAL_METRIC.get_or_init(|| {
        meter("hc.ribosome")
            .u64_counter("hc.ribosome.host_fn.send_remote_signal")
            .with_description("The number of remote signals sent.")
            .build()
    })
}
