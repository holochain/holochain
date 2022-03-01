use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::event::GetRequest;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_header<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetHeaderInput,
) -> Result<SignedHeaderHashed, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let header_hash = input.into_inner();

            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                let mut cascade = match call_context.host_context {
                    HostContext::Validate(_) => Cascade::from_workspace(workspace.stores(), None),
                    _ => Cascade::from_workspace_network(&workspace, call_context.host_context.network().clone()),
                };
                match cascade
                    .retrieve_header(
                        header_hash.clone(),
                        // Set every GetOptions manually here.
                        // Using defaults is dangerous in a must_get as it can undermine determinism.
                        // We want refactors to explicitly consider this.
                        NetworkGetOptions {
                            remote_agent_count: None,
                            timeout_ms: None,
                            as_race: true,
                            race_timeout_ms: None,
                            // Never redirect as the returned entry must always match the hash.
                            follow_redirects: false,
                            all_live_headers_with_metadata: false,
                            // Redundant with retrieve_entry internals.
                            request_type: GetRequest::Pending,
                        },
                    )
                    .await
                    .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))?
                {
                    Some(header) => Ok(header),
                    None => match call_context.host_context {
                        HostContext::EntryDefs(_)
                        | HostContext::GenesisSelfCheck(_)
                        | HostContext::MigrateAgent(_)
                        | HostContext::PostCommit(_)
                        | HostContext::ZomeCall(_) => Err(WasmError::Host(format!(
                            "Failed to get SignedHeaderHashed {}",
                            header_hash
                        ))),
                        HostContext::Init(_) => RuntimeError::raise(Box::new(
                            WasmError::HostShortCircuit(holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    vec![header_hash.into()],
                                ))?,
                            )?),
                        )),
                        HostContext::Validate(_) => RuntimeError::raise(Box::new(
                            WasmError::HostShortCircuit(holochain_serialized_bytes::encode(
                                &ExternIO::encode(ValidateCallbackResult::UnresolvedDependencies(
                                    vec![header_hash.into()],
                                ))?,
                            )?),
                        )),
                        HostContext::ValidationPackage(_) => {
                            RuntimeError::raise(Box::new(WasmError::HostShortCircuit(
                                holochain_serialized_bytes::encode(&ExternIO::encode(
                                    ValidationPackageCallbackResult::UnresolvedDependencies(vec![
                                        header_hash.into(),
                                    ]),
                                )?)?,
                            )))
                        }
                    },
                }
            })
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "must_get_header".into(),
            )
            .to_string(),
        )),
    }
}
