use holochain_wasmer_host::prelude::*;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::event::GetRequest;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_header<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetHeaderInput,
) -> Result<SignedHeaderHashed, WasmError> {
    let header_hash = input.into_inner();
    let network = call_context.host_context.network().clone();

    // timeouts must be handled by the network
    tokio_helper::block_forever_on(async move {
        let workspace = call_context.host_context.workspace();
        let mut cascade = Cascade::from_workspace_network(workspace, network);
        match cascade
            .retrieve_header(header_hash.clone(), // Set every GetOptions manually here.
            // Using defaults is dangerous as it can undermine determinism.
            // We want refactors to explicitly consider this.
            NetworkGetOptions {
                remote_agent_count: None,
                timeout_ms: None,
                as_race: true,
                race_timeout_ms: None,
                // Never redirect as the returned entry must always match the hash.
                follow_redirects: false,
                // Ignore deletes.
                all_live_headers_with_metadata: true,
                // Redundant with retrieve_entry internals.
                request_type: GetRequest::Pending,
            })
            .await
            .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))? {
                Some(header) => Ok(header),
                None => match call_context.host_context {
                    HostContext::EntryDefs(_) | HostContext::GenesisSelfCheck(_) | HostContext::MigrateAgent(_) | HostContext::PostCommit(_) | HostContext::ZomeCall(_) => Err(WasmError::Host(format!("Failed to get SignedHeaderHashed {}", header_hash))),
                    HostContext::Init(_) => RuntimeError::raise(
                        Box::new(
                            WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &Ok::<InitCallbackResult, ()>(InitCallbackResult::UnresolvedDependencies(vec![header_hash.into()]))
                                )?
                            )
                        )
                    ),
                    HostContext::ValidateCreateLink(_) => RuntimeError::raise(
                        Box::new(
                            WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &Ok::<ValidateLinkCallbackResult, ()>(ValidateLinkCallbackResult::UnresolvedDependencies(vec![header_hash.into()]))
                                )?
                            )
                        )
                    ),
                    HostContext::Validate(_) => RuntimeError::raise(
                        Box::new(
                            WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &Ok::<ValidateCallbackResult, ()>(ValidateCallbackResult::UnresolvedDependencies(vec![header_hash.into()]))
                                )?
                            )
                        )
                    ),
                    HostContext::ValidationPackage(_) => RuntimeError::raise(
                        Box::new(
                            WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &Ok::<ValidationPackageCallbackResult, ()>(ValidationPackageCallbackResult::UnresolvedDependencies(vec![header_hash.into()]))
                                )?
                            )
                        )
                    ),
                },
            }
    })
}