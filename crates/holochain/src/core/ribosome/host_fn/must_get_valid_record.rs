use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::GetOptions;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn must_get_valid_record<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetValidRecordInput,
) -> Result<Record, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let action_hash = input.into_inner();

            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                let mut cascade = match call_context.host_context {
                    HostContext::Validate(_) => Cascade::from_workspace(workspace.stores(), None),
                    _ => Cascade::from_workspace_network(
                        &workspace,
                        call_context.host_context.network().clone(),
                    ),
                };
                match cascade
                    .get_action_details(action_hash.clone(), GetOptions::content())
                    .await
                    .map_err(|cascade_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                    })? {
                    Some(RecordDetails {
                        record,
                        validation_status: ValidationStatus::Valid,
                        ..
                    }) => Ok(record),
                    _ => match call_context.host_context {
                        HostContext::EntryDefs(_)
                        | HostContext::GenesisSelfCheck(_)
                        | HostContext::MigrateAgent(_)
                        | HostContext::PostCommit(_)
                        | HostContext::Weigh(_)
                        | HostContext::ZomeCall(_) => Err(wasm_error!(WasmErrorInner::Host(
                            format!("Failed to get Record {}", action_hash)
                        ))
                        .into()),
                        HostContext::Init(_) => Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    vec![action_hash.into()],
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?
                        ))
                        .into()),
                        HostContext::Validate(_) => {
                            Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(
                                        ValidateCallbackResult::UnresolvedDependencies(vec![
                                            action_hash.into()
                                        ],)
                                    )
                                    .map_err(
                                        |e| -> RuntimeError { wasm_error!(e.into()).into() }
                                    )?,
                                )
                                .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?
                            ))
                            .into())
                        }
                        HostContext::ValidationPackage(_) => {
                            Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(
                                        ValidationPackageCallbackResult::UnresolvedDependencies(
                                            vec![action_hash.into(),]
                                        ),
                                    )
                                    .map_err(
                                        |e| -> RuntimeError { wasm_error!(e.into()).into() }
                                    )?
                                )
                                .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?,
                            ))
                            .into())
                        }
                    },
                }
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "must_get_valid_record".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}
