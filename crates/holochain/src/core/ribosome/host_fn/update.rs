use super::create::extract_entry_def;
use super::delete::get_original_entry_data;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::WasmError;

use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn update<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: UpdateInput,
) -> Result<HeaderHash, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            // destructure the args out into an app type def id and entry
            let UpdateInput {
                original_header_address,
                entry_def_id,
                entry,
                chain_top_ordering,
            } = input;

            let (original_entry_address, entry_type) =
                get_original_entry_data(call_context.clone(), original_header_address.clone())?;

            let zome = match entry_type {
                EntryType::App(AppEntryType { zome_id, .. }) => {
                    let zome = ribosome
                    .dna_def()
                    .integrity_zomes
                    .get(zome_id.index())
                    .cloned()
                    .map(Zome::from)
                    .ok_or_else(|| WasmError::Host(format!("Tried to delete an entry from ZomeId {} which is not an integrity zome", zome_id)))?;
                    Some(zome)
                }
                _ => None,
            };

            // Countersigned entries have different header handling.
            match entry {
                Entry::CounterSign(_, _) => tokio_helper::block_forever_on(async move {
                    call_context
                        .host_context
                        .workspace_write()
                        .source_chain()
                        .as_ref()
                        .expect("Must have source chain if write_workspace access is given")
                        .put_countersigned(None, entry, chain_top_ordering)
                        .await
                        .map_err(|source_chain_error| {
                            WasmError::Host(source_chain_error.to_string())
                        })
                }),
                _ => {
                    // build the entry hash
                    let entry_hash = EntryHash::with_data_sync(&entry);

                    // extract the entry defs for a zome
                    let entry_type = match entry_def_id {
                        EntryDefId::App(entry_def_id) => {
                            let zome = zome
                                .as_ref()
                                .expect("Zome can never be none for a EntryDefLocation::App");
                            // extract the zome position
                            let header_zome_id =
                                ribosome.zome_name_to_id(zome.zome_name()).map_err(|source_chain_error| {
                                    WasmError::Host(source_chain_error.to_string())
                                })?;
                            let (header_entry_def_id, entry_visibility) = extract_entry_def(
                                ribosome,
                                call_context.clone(),
                                zome.clone(),
                                entry_def_id.into(),
                            )?;
                            let app_entry_type = AppEntryType::new(
                                header_entry_def_id,
                                header_zome_id,
                                entry_visibility,
                            );
                            EntryType::App(app_entry_type)
                        }
                        EntryDefId::CapGrant => EntryType::CapGrant,
                        EntryDefId::CapClaim => EntryType::CapClaim,
                    };

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
                            .put(None, header_builder, Some(entry), chain_top_ordering)
                            .await
                            .map_err(|source_chain_error| {
                                WasmError::Host(source_chain_error.to_string())
                            })?;
                        Ok(header_hash)
                    })
                }
            }
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "update".into(),
            )
            .to_string(),
        )),
    }
}

// relying on tests for get_details
