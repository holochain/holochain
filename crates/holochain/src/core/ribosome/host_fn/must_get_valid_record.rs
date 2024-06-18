use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::CascadeImpl;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[allow(clippy::extra_unused_lifetimes)]
#[tracing::instrument(skip(_ribosome, call_context))]
pub fn must_get_valid_record<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetValidRecordInput,
) -> Result<Record, RuntimeError> {
    tracing::debug!("begin must_get_valid_record");
    let ret = match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let action_hash = input.into_inner();

            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                use crate::core::ribosome::ValidateHostAccess;
                let (cascade, opt) = match call_context.host_context {
                    HostContext::Validate(ValidateHostAccess { is_inline, .. }) => {

                        if is_inline {
                            (
                                CascadeImpl::from_workspace_and_network(
                                    &workspace,
                                    call_context.host_context.network().clone(),
                                ),
                                GetOptions::network(),
                            )
                        } else {
                            (
                                CascadeImpl::from_workspace_stores(workspace.stores(), None),
                                GetOptions::local(),
                            )
                        }
                    }
                    _ => (
                        CascadeImpl::from_workspace_and_network(
                            &workspace,
                            call_context.host_context.network().clone(),
                        ),
                        GetOptions::local(),
                    ),
                };
                match cascade
                    .get_record_details(action_hash.clone(), opt)
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
                        | HostContext::GenesisSelfCheckV1(_)
                        | HostContext::GenesisSelfCheckV2(_)
                        | HostContext::MigrateAgent(_)
                        | HostContext::PostCommit(_)
                        | HostContext::ZomeCall(_) => Err(wasm_error!(WasmErrorInner::Host(
                            format!("Failed to get Record {}", action_hash)
                        ))
                        .into()),
                        HostContext::Init(_) => Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    UnresolvedDependencies::Hashes(vec![action_hash.into()],)
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
                        ))
                        .into()),
                        HostContext::Validate(_) => {
                            Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                                holochain_serialized_bytes::encode(
                                    &ExternIO::encode(
                                        ValidateCallbackResult::UnresolvedDependencies(
                                            UnresolvedDependencies::Hashes(
                                                vec![action_hash.into()],
                                            )
                                        )
                                    )
                                    .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                                )
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
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
    };
    tracing::debug!(?ret);
    ret
}
