use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::error::CascadeResult;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn delete_link<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: DeleteLinkInput,
) -> Result<HeaderHash, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let DeleteLinkInput {
                address,
                chain_top_ordering,
            } = input;
            // get the base address from the add link header
            // don't allow the wasm developer to get this wrong
            // it is never valid to have divergent base address for add/remove links
            // the subconscious will validate the base address match but we need to fetch it here to
            // include it in the remove link header
            let network = call_context.host_context.network().clone();
            let call_context_2 = call_context.clone();

            // handle timeouts at the network layer
            let address_2 = address.clone();
            let maybe_add_link: Option<SignedHeaderHashed> =
                tokio_helper::block_forever_on(async move {
                    let workspace = call_context_2.host_context.workspace();
                    CascadeResult::Ok(
                        Cascade::from_workspace_network(&workspace, network)
                            .dht_get(address_2.into(), GetOptions::content())
                            .await?
                            .map(|el| el.into_inner().0),
                    )
                })
                .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))?;

            let base_address = match maybe_add_link {
                Some(add_link_signed_header_hash) => {
                    match add_link_signed_header_hash.header() {
                        Header::CreateLink(link_add_header) => {
                            Ok(link_add_header.base_address.clone())
                        }
                        // the add link header hash provided was found but didn't point to an AddLink
                        // header (it is something else) so we cannot proceed
                        _ => Err(RibosomeError::ElementDeps(address.clone().into())),
                    }
                }
                // the add link header hash could not be found
                // it's unlikely that a wasm call would have a valid add link header hash from "somewhere"
                // that isn't also discoverable in either the cache or DHT, but it _is_ possible so we have
                // to fail in that case (e.g. the local cache could have GC'd at the same moment the
                // network connection dropped out)
                None => Err(RibosomeError::ElementDeps(address.clone().into())),
            }
            .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))?;

            let source_chain = call_context
                .host_context
                .workspace_write()
                .source_chain()
                .as_ref()
                .expect("Must have source chain if write_workspace access is given");
            let zome = call_context.zome.clone();

            // handle timeouts at the source chain layer

            // add a DeleteLink to the source chain
            tokio_helper::block_forever_on(async move {
                let header_builder = builder::DeleteLink {
                    link_add_address: address,
                    base_address,
                };
                let header_hash = source_chain
                    .put(Some(zome), header_builder, None, chain_top_ordering)
                    .await
                    .map_err(|source_chain_error| {
                        WasmError::Host(source_chain_error.to_string())
                    })?;
                Ok(header_hash)
            })
        }
        _ => Err(WasmError::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "delete_link".into()
        ).to_string()))
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use hdk::prelude::*;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holo_hash::HeaderHash;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_delete_link_add_remove() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        // links should start empty
        let links: Vec<Vec<Link>> = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ()).unwrap();

        assert!(links.len() == 0);

        // add a couple of links
        let link_one: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "create_link", ()).unwrap();

        // add a couple of links
        let link_two: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "create_link", ()).unwrap();

        let links: Vec<Link> = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ()).unwrap();

        assert!(links.len() == 2);

        // remove a link
        let _: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "delete_link", link_one)
                .unwrap();

        let links: Vec<Link> = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ()).unwrap();

        assert!(links.len() == 1);

        // remove a link
        let _: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "delete_link", link_two)
                .unwrap();

        let links: Vec<Link> = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ()).unwrap();

        assert!(links.len() == 0);

        // Add some links then delete them all
        let _h: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "create_link", ()).unwrap();
        let _h: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "create_link", ()).unwrap();

        let links: Vec<Link> = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ()).unwrap();

        assert!(links.len() == 2);

        let _: () = crate::call_test_ribosome!(host_access, TestWasm::Link, "delete_all_links", ())
            .unwrap();

        // Should be no links left
        let links: Vec<Link> = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ()).unwrap();

        assert!(links.len() == 0);
    }
}
