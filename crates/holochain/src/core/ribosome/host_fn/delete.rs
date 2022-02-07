use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::error::CascadeError;
use holochain_cascade::Cascade;
use holochain_wasmer_host::prelude::WasmError;

use crate::core::ribosome::HostFnAccess;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn delete<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: DeleteInput,
) -> Result<HeaderHash, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let DeleteInput {
                deletes_header_hash,
                chain_top_ordering,
            } = input;
            let deletes_entry_address =
                get_original_address(call_context.clone(), deletes_header_hash.clone())?;

            let host_access = call_context.host_context();

            // handle timeouts at the source chain layer
            tokio_helper::block_forever_on(async move {
                let source_chain = host_access
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given");
                let header_builder = builder::Delete {
                    deletes_address: deletes_header_hash,
                    deletes_entry_address,
                };
                let header_hash = source_chain
                    .put(
                        Some(call_context.zome.clone()),
                        header_builder,
                        None,
                        chain_top_ordering,
                    )
                    .await
                    .map_err(|source_chain_error| {
                        WasmError::Host(source_chain_error.to_string())
                    })?;
                Ok(header_hash)
            })
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "delete".into(),
            )
            .to_string(),
        )),
    }
}

#[allow(clippy::extra_unused_lifetimes)]
pub(crate) fn get_original_address<'a>(
    call_context: Arc<CallContext>,
    address: HeaderHash,
) -> Result<EntryHash, WasmError> {
    let network = call_context.host_context.network().clone();
    let workspace = call_context.host_context.workspace();

    tokio_helper::block_forever_on(async move {
        let mut cascade = Cascade::from_workspace_network(&workspace, network);
        let maybe_original_element: Option<SignedHeaderHashed> = cascade
            .get_details(address.clone().into(), GetOptions::content())
            .await?
            .map(|el| {
                match el {
                    holochain_zome_types::metadata::Details::Element(e) => {
                        Ok(e.element.into_inner().0)
                    }
                    // Should not be trying to get original headers via EntryHash
                    holochain_zome_types::metadata::Details::Entry(_) => {
                        Err(CascadeError::InvalidResponse(address.clone().into()))
                    }
                }
            })
            .transpose()?;

        match maybe_original_element {
            Some(original_element_signed_header_hash) => {
                match original_element_signed_header_hash.header().entry_data() {
                    Some((entry_hash, _)) => Ok(entry_hash.clone()),
                    _ => Err(RibosomeError::ElementDeps(address.into())),
                }
            }
            None => Err(RibosomeError::ElementDeps(address.into())),
        }
    })
    .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_delete_entry_test<'a>() {
        observability::test_run().ok();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        let thing_a: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crd, "create", ()).unwrap();
        let get_thing: Option<Element> =
            crate::call_test_ribosome!(host_access, TestWasm::Crd, "reed", thing_a).unwrap();
        match get_thing {
            Some(element) => assert!(element.entry().as_option().is_some()),

            None => unreachable!(),
        }

        let _: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crd, "delete", thing_a).unwrap();

        let get_thing: Option<Element> =
            crate::call_test_ribosome!(host_access, TestWasm::Crd, "reed", thing_a).unwrap();
        match get_thing {
            None => {
                // this is what we want, deletion => None for a get
            }
            _ => unreachable!(),
        }
    }
}
