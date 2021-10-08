use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::info::CallInfo;
use holochain_types::prelude::Permission;
use holochain_types::prelude::HostFnAccess;

pub fn call_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: (),
) -> Result<CallInfo, WasmError> {
    match HostFnAccess::from(call_context.host_context()) {
        HostFnAccess { bindings: Permission::Allow, .. } => {
            Ok(call_context.call_info().clone())
        },
        _ => unreachable!(),
    }
}

