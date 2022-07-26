use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetActivityOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

pub fn must_get_agent_activity(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetAgentActivityInput,
) -> Result<Vec<RegisterAgentActivity>, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let MustGetAgentActivityInput {
                author,
                chain_filter,
            } = input;

            // Get the network from the context
            let network = call_context.host_context.network().clone();

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
                let result: Result<_, RuntimeError> = match cascade
                    .must_get_agent_activity(author, chain_filter)
                    .await
                    .map_err(|cascade_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                    })? {
                    MustGetAgentActivityResponse::Activity(activity) => Ok(activity),
                    MustGetAgentActivityResponse::IncompleteChain => todo!(),
                    MustGetAgentActivityResponse::ActionNotFound(_) => todo!(),
                    MustGetAgentActivityResponse::PositionNotHighest => todo!(),
                    MustGetAgentActivityResponse::EmptyRange => todo!(),
                    // Some(activity) => Ok(activity),
                    // None => match call_context.host_context {
                    //     HostContext::EntryDefs(_)
                    //     | HostContext::GenesisSelfCheck(_)
                    //     | HostContext::MigrateAgent(_)
                    //     | HostContext::PostCommit(_)
                    //     | HostContext::ZomeCall(_) => Err(wasm_error!(WasmErrorInner::Host(
                    //         format!("Failed to get EntryHashed {}", entry_hash)
                    //     ))
                    //     .into()),
                    //     HostContext::ValidationPackage(_)
                    //     | HostContext::Validate(_)
                    //     | HostContext::Init(_) => {
                    //         Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                    //             holochain_serialized_bytes::encode(
                    //                 &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                    //                     vec![entry_hash.into()],
                    //                 ))
                    //                 .map_err(
                    //                     |e| -> RuntimeError { wasm_error!(e.into()).into() }
                    //                 )?,
                    //             )
                    //             .map_err(|e| -> RuntimeError { wasm_error!(e.into()).into() })?
                    //         ))
                    //         .into())
                    //     }
                    // },
                };
                result
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "must_get_agent_activity".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

// we are relying on the create tests to show the commit/get round trip
// See commit_entry.rs
