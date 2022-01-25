use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::CallContext;
use holochain_types::signal::Signal;
use holochain_types::prelude::*;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;

pub fn emit_signal(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: AppSignal,
) -> Result<(), WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ write_workspace: Permission::Allow, .. } => {
            let cell_id = CellId::new(
                ribosome.dna_def().as_hash().clone(),
                call_context.host_context.workspace().source_chain().as_ref().expect("Must have a source chain to emit signals").agent_pubkey().clone(),
            );
            // call_context.host_context().cell_id().clone();
            let signal = Signal::App(cell_id, input);
            call_context.host_context().signal_tx().send(signal).map_err(|interface_error| WasmError::Host(interface_error.to_string()))?;
            Ok(())
        },
        _ => Err(WasmError::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "emit_signal".into()
        ).to_string()))
    }
}
