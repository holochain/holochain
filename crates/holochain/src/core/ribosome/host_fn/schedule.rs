use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_types::prelude::*;

pub fn schedule(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: String,
) -> Result<(), WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ write_workspace: Permission::Allow, .. } => {
            call_context.host_context().workspace().source_chain().scratch().apply(|scratch| {
                scratch.add_scheduled_fn(input);
            }).map_err(|e| WasmError::Host(e.to_string()))?;
            Ok(())
        },
        _ => unreachable!(),
    }
}