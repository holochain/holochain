use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::{error::RibosomeResult, CallContext};
use holochain_types::signal::Signal;
use holochain_types::prelude::*;
use std::sync::Arc;
use super::HostFnMetrics;

pub fn emit_signal(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: AppSignal,
) -> RibosomeResult<()> {
    HostFnMetrics::count(HostFnMetrics::EmitSignal, 1);
    let cell_id = call_context.host_access().cell_id().clone();
    let signal = Signal::App(cell_id, input);
    call_context.host_access().signal_tx().send(signal)?;
    Ok(())
}
