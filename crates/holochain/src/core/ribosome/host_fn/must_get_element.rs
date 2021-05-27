use holochain_wasmer_host::prelude::*;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use holochain_zome_types::GetOptions;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_element<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetElementInput,
) -> Result<(Element, bool), WasmError> {
    let header_hash = input.into_inner();
    let network = call_context.host_context.network().clone();

    // timeouts must be handled by the network
    tokio_helper::block_forever_on(async move {
        let workspace = call_context.host_context.workspace();
        let mut cascade = Cascade::from_workspace_network(workspace, network);
        match cascade
            .get_header_details(header_hash.clone(),
            GetOptions::content())
            .await
            .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))? {
                Some(ElementDetails{ element, validation_status: ValidationStatus::Valid, ..}) => Ok((element, true)),
                Some(ElementDetails{ element, validation_status: ValidationStatus::Rejected, ..}) => Ok((element, false)),
                _ => RuntimeError::raise(Box::new(WasmError::HostShortCircuit(match call_context.host_context {
                    HostContext::EntryDefs(_) | HostContext::GenesisSelfCheck(_) | HostContext::MigrateAgent(_) | HostContext::PostCommit(_) | HostContext::ZomeCall(_) => holochain_serialized_bytes::encode(&Err::<(), WasmError>(WasmError::Host("Missing dep".into())))?,
                    HostContext::Init(_) => holochain_serialized_bytes::encode(&Ok::<InitCallbackResult, ()>(InitCallbackResult::UnresolvedDependencies(vec![header_hash.into()])))?,
                    HostContext::ValidateCreateLink(_) => holochain_serialized_bytes::encode(&Ok::<ValidateLinkCallbackResult, ()>(ValidateLinkCallbackResult::UnresolvedDependencies(vec![header_hash.into()])))?,
                    HostContext::Validate(_) => holochain_serialized_bytes::encode(&Ok::<ValidateCallbackResult, ()>(ValidateCallbackResult::UnresolvedDependencies(vec![header_hash.into()])))?,
                    HostContext::ValidationPackage(_) => holochain_serialized_bytes::encode(&Ok::<ValidationPackageCallbackResult, ()>(ValidationPackageCallbackResult::UnresolvedDependencies(vec![header_hash.into()])))?,
                }))),
            }
    })
}