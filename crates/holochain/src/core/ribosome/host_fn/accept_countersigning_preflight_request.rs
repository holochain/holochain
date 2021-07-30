use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;

#[allow(clippy::extra_unused_lifetimes)]
pub fn accept_countersigning_preflight_request<'a>(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: PreflightRequest,
) -> Result<PreflightRequestAcceptance, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ agent_info: Permission::Allow, keystore: Permission::Allow, non_determinism: Permission::Allow, .. } => {
            tokio_helper::block_forever_on(async move {
                call_context.host_context.workspace().source_chain().accept_countersigning_preflight_request(input).await
            })
        },
        _ => unreachable!(),
    }
}