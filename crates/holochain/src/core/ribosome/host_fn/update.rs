use super::delete::get_original_entry_data;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::*;

use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn update<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: UpdateInput,
) -> Result<HeaderHash, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            // destructure the args out into an app type def id and entry
            let UpdateInput {
                original_header_address,
                entry,
                chain_top_ordering,
            } = input;

            let (original_entry_address, entry_type) =
                get_original_entry_data(call_context.clone(), original_header_address.clone())?;

            let weight = todo!("weigh element");

            let weight = todo!("weigh element");

            // Countersigned entries have different header handling.
            match entry {
                Entry::CounterSign(_, _) => tokio_helper::block_forever_on(async move {
                    call_context
                        .host_context
                        .workspace_write()
                        .source_chain()
                        .as_ref()
                        .expect("Must have source chain if write_workspace access is given")
                        .put_countersigned(entry, chain_top_ordering, weight)
                        .await
                        .map_err(|source_chain_error| -> RuntimeError {
                            wasm_error!(WasmErrorInner::Host(source_chain_error.to_string())).into()
                        })
                }),
                _ => {
                    // build the entry hash
                    let entry_hash = EntryHash::with_data_sync(&entry);

                    // build a header for the entry being updated
                    let header_builder = builder::Update {
                        original_entry_address,
                        original_header_address,
                        entry_type,
                        entry_hash,
                    };
                    let workspace = call_context.host_context.workspace_write();

                    // return the hash of the updated entry
                    // note that validation is handled by the workflow
                    // if the validation fails this update will be rolled back by virtue of the DB transaction
                    // being atomic
                    tokio_helper::block_forever_on(async move {
                        let source_chain = workspace
                            .source_chain()
                            .as_ref()
                            .expect("Must have source chain if write_workspace access is given");
                        // push the header and the entry into the source chain
                        let header_hash = source_chain
                            .put_weightless(header_builder, Some(entry), chain_top_ordering)
                            .await
                            .map_err(|source_chain_error| -> RuntimeError {
                                wasm_error!(WasmErrorInner::Host(source_chain_error.to_string()))
                                    .into()
                            })?;
                        Ok(header_hash)
                    })
                }
            }
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "update".into()
            )
            .to_string()
        ))
        .into()),
    }
}

// relying on tests for get_details
