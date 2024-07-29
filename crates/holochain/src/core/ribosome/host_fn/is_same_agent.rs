use crate::core::ribosome::guest_callback::validate::ValidateHostAccess;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[allow(clippy::extra_unused_lifetimes)]
pub fn is_same_agent<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: (AgentPubKey, AgentPubKey),
) -> Result<bool, RuntimeError> {
    match &call_context.host_context() {
        HostContext::Validate(ValidateHostAccess { dpki, .. }) => {
            match dpki {
                // If DPKI is not installed, compare input agent keys directly. Returns `true` only
                // when agent keys are identical.
                None => Ok(input.0 == input.1),
                // DPKI is installed, call Deepkey function.
                Some(dpki) => tokio_helper::block_forever_on(async move {
                    let state = dpki.state().await;
                    state
                        .is_same_agent(input.0, input.1)
                        .await
                        .map_err(|error| RuntimeError::new(error.to_string()))
                        .into()
                }),
            }
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "is_same_agent".into()
            )
            .to_string()
        ))
        .into()),
    }
}
