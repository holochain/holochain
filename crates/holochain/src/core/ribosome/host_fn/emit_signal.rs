use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_types::signal::Signal;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

pub fn emit_signal(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: AppSignal,
) -> Result<(), WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let cell_id = call_context.host_context().cell_id().clone();
            let signal = Signal::App(cell_id, input);
            call_context
                .host_context()
                .signal_tx()
                .send(signal)
                .map_err(|interface_error| wasm_error!(WasmErrorInner::Host(interface_error.to_string())))?;
            Ok(())
        }
        _ => unreachable!(),
    }
}
