use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::error::CascadeError;
use holochain_cascade::Cascade;
use holochain_wasmer_host::prelude::*;

use crate::core::ribosome::HostFnAccess;
use holo_hash::ActionHash;
use holo_hash::EntryHash;
use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn delete<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: DeleteInput,
) -> Result<ActionHash, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let DeleteInput {
                deletes_action_hash,
                chain_top_ordering,
            } = input;
            let (deletes_entry_address, _) =
                get_original_entry_data(call_context.clone(), deletes_action_hash.clone())?;

            let host_access = call_context.host_context();

            // handle timeouts at the source chain layer
            tokio_helper::block_forever_on(async move {
                let source_chain = host_access
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given");
                let action_builder = builder::Delete {
                    deletes_address: deletes_action_hash,
                    deletes_entry_address,
                };
                let action_hash = source_chain
                    .put_weightless(action_builder, None, chain_top_ordering)
                    .await
                    .map_err(|source_chain_error| {
                        wasm_error!(WasmErrorInner::Host(source_chain_error.to_string()))
                    })?;
                Ok(action_hash)
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "delete".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

pub(crate) fn get_original_entry_data(
    call_context: Arc<CallContext>,
    address: ActionHash,
) -> Result<(EntryHash, EntryType), WasmError> {
    let network = call_context.host_context.network().clone();
    let workspace = call_context.host_context.workspace();

    tokio_helper::block_forever_on(async move {
        let mut cascade = Cascade::from_workspace_network(&workspace, network);
        let maybe_original_record: Option<SignedActionHashed> = cascade
            .get_details(address.clone().into(), GetOptions::content())
            .await?
            .map(|el| {
                match el {
                    holochain_zome_types::metadata::Details::Record(e) => {
                        Ok(e.record.into_inner().0)
                    }
                    // Should not be trying to get original actions via EntryHash
                    holochain_zome_types::metadata::Details::Entry(_) => {
                        Err(CascadeError::InvalidResponse(address.clone().into()))
                    }
                }
            })
            .transpose()?;

        match maybe_original_record {
            Some(SignedActionHashed {
                hashed: ActionHashed {
                    content: action, ..
                },
                ..
            }) => match action.into_entry_data() {
                Some((entry_hash, entry_type)) => Ok((entry_hash, entry_type)),
                _ => Err(RibosomeError::RecordDeps(address.into())),
            },
            None => Err(RibosomeError::RecordDeps(address.into())),
        }
    })
    .map_err(|ribosome_error| wasm_error!(WasmErrorInner::Host(ribosome_error.to_string())))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_delete_entry_test<'a>() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Crd).await;

        let thing_a: ActionHash = conductor.call(&alice, "create", ()).await;
        let get_thing: Option<Record> = conductor.call(&alice, "reed", thing_a.clone()).await;
        match get_thing {
            Some(record) => assert!(record.entry().as_option().is_some()),

            None => unreachable!(),
        }

        let _: ActionHash = conductor
            .call(&alice, "delete_via_hash", thing_a.clone())
            .await;

        let get_thing: Option<Record> = conductor.call(&alice, "reed", thing_a).await;
        match get_thing {
            None => {
                // this is what we want, deletion => None for a get
            }
            _ => unreachable!(),
        }
    }
}
