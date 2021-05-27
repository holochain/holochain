use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_header<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetHeaderInput,
) -> Result<Header, WasmError> {
    // Get the network from the context
    let network = call_context.host_access.network().clone();

    // timeouts must be handled by the network
    tokio_helper::block_forever_on(async move {
        let workspace = call_context.host_access.workspace();
        let mut cascade = Cascade::from_workspace_network(workspace, network);
        match cascade
            .retrieve_header(input.into_inner(), GetOptions::content())
            .await
            .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))? {
                Some(header) => Ok(header),
                None => RibosomeError::raise(WasmError::HostShortCircuit(holochain_serialized_bytes::encode(&match call_context.host_context {
                    HostContext::EntryDefs(_) | HostContext::GenesisSelfCheck(_) | HostContext::MigrateAgent(_) | HostContext::PostCommit(_) | HostContext::ZomeCall(_) => Err(WasmError::Host("Missing dep".into())),
                    HostContext::Init(_) => Ok(InitCallbackResult::UnresolvedDependencies(call_context.zome.name, vec![header_hash])),
                    HostContext::ValidateCreateLink(_) => Ok(ValidateLinkCallbackResult::UnresolvedDependencies(call_context.zome.name, vec![header_hash])),
                    HostContext::Validate(_) => Ok(ValidateCallbackResult::UnresolvedDependencies(call_context.zome.name, vec![header_hash])),
                    HostContext::ValidationPackage(_) => Ok(ValidationPackageCallbackResult::UnresolvedDependencies(call_context.zome.name, vec![header_hash])),
                })?)),
            }
        Ok(maybe_element)
    })
}

// we are relying on the create tests to show the commit/get round trip
// See commit_entry.rs