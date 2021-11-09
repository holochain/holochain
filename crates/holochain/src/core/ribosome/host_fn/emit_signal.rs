use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::CallContext;
use holochain_types::signal::Signal;
use holochain_types::prelude::*;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use crate::core::ribosome::HostFnAccess;

pub fn emit_signal(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: AppSignal,
) -> Result<(), WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ write_workspace: Permission::Allow, .. } => {
            let cell_id = CellId::new(
                ribosome.dna_def().as_hash().clone(),
                call_context.host_context.workspace().source_chain().agent_pubkey().clone(),
            );
            // call_context.host_context().cell_id().clone();
            let signal = Signal::App(cell_id, input);
            call_context.host_context().signal_tx().send(signal).map_err(|interface_error| WasmError::Host(interface_error.to_string()))?;
            Ok(())
        },
        _ => unreachable!(),
    }
}
