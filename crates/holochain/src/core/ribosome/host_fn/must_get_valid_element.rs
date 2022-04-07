use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::GetOptions;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_valid_element<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetValidElementInput,
) -> Result<Element, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let header_hash = input.into_inner();
            let network = call_context.host_context.network().clone();

            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                let mut cascade = Cascade::from_workspace_network(&workspace, network);
                match cascade
                    .get_header_details(header_hash.clone(), GetOptions::content())
                    .await
                    .map_err(|cascade_error| wasm_error!(WasmErrorInner::Host(cascade_error.to_string())))?
                {
                    Some(ElementDetails {
                        element,
                        validation_status: ValidationStatus::Valid,
                        ..
                    }) => Ok(element),
                    _ => match call_context.host_context {
                        HostContext::EntryDefs(_)
                        | HostContext::GenesisSelfCheck(_)
                        | HostContext::MigrateAgent(_)
                        | HostContext::PostCommit(_)
                        | HostContext::ZomeCall(_) => Err(wasm_error!(WasmErrorInner::Host(format!(
                            "Failed to get Element {}",
                            header_hash
                        )))),
                        HostContext::Init(_) => RuntimeError::raise(Box::new(wasm_error!(
                            WasmErrorInner::HostShortCircuit(holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    vec![header_hash.into()],
                                ))
                                .map_err(|e| wasm_error!(e.into()))?,
                            ).map_err(|e| wasm_error!(e.into()))?)
                        ))),
                        HostContext::ValidateCreateLink(_) => RuntimeError::raise(Box::new(
                            wasm_error!(WasmErrorInner::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(
                                        ValidateLinkCallbackResult::UnresolvedDependencies(vec![
                                            header_hash.into(),
                                        ]),
                                    )
                                    .map_err(|e| wasm_error!(e.into()))?
                                ).map_err(|e| wasm_error!(e.into()))?,
                            )),
                        )),
                        HostContext::Validate(_) => RuntimeError::raise(Box::new(wasm_error!(
                            WasmErrorInner::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(
                                        ValidateCallbackResult::UnresolvedDependencies(vec![
                                            header_hash.into()
                                        ],)
                                    )
                                    .map_err(|e| wasm_error!(e.into()))?,
                                )
                                .map_err(|e| wasm_error!(e.into()))?
                            )
                        ))),
                        HostContext::ValidationPackage(_) => RuntimeError::raise(Box::new(
                            wasm_error!(WasmErrorInner::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(
                                        ValidationPackageCallbackResult::UnresolvedDependencies(
                                            vec![header_hash.into()]
                                        )
                                    )
                                    .map_err(|e| wasm_error!(e.into()))?
                                )
                                .map_err(|e| wasm_error!(e.into()))?
                            )),
                        )),
                    },
                }
            })
        }
        _ => unreachable!(),
    }
}
