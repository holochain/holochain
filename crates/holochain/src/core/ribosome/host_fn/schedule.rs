use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_types::prelude::*;

pub fn schedule(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: String,
) -> Result<(), WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ write_workspace: Permission::Allow, .. } => {
            unimplemented!()
        },
        _ => unreachable!(),
    }
}