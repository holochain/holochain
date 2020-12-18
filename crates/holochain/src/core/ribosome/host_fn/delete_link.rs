use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_to_authored;
use holochain_cascade::error::CascadeResult;
use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn delete_link<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: DeleteLinkInput,
) -> RibosomeResult<DeleteLinkOutput> {
    let link_add_address = input.into_inner();

    // get the base address from the add link header
    // don't allow the wasm developer to get this wrong
    // it is never valid to have divergent base address for add/remove links
    // the subconscious will validate the base address match but we need to fetch it here to
    // include it in the remove link header
    let network = call_context.host_access.network().clone();
    let address = link_add_address.clone();
    let call_context_2 = call_context.clone();

    // handle timeouts at the network layer
    let maybe_add_link: Option<SignedHeaderHashed> =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            CascadeResult::Ok(
                call_context_2
                    .clone()
                    .host_access
                    .workspace()
                    .write()
                    .await
                    .cascade(network)
                    .dht_get(address.into(), GetOptions::content())
                    .await?
                    .map(|el| el.into_inner().0),
            )
        })?;

    let base_address = match maybe_add_link {
        Some(add_link_signed_header_hash) => {
            match add_link_signed_header_hash.header() {
                Header::CreateLink(link_add_header) => Ok(link_add_header.base_address.clone()),
                // the add link header hash provided was found but didn't point to an AddLink
                // header (it is something else) so we cannot proceed
                _ => Err(RibosomeError::ElementDeps(link_add_address.clone().into())),
            }
        }
        // the add link header hash could not be found
        // it's unlikely that a wasm call would have a valid add link header hash from "somewhere"
        // that isn't also discoverable in either the cache or DHT, but it _is_ possible so we have
        // to fail in that case (e.g. the local cache could have GC'd at the same moment the
        // network connection dropped out)
        None => Err(RibosomeError::ElementDeps(link_add_address.clone().into())),
    }?;

    let workspace_lock = call_context.host_access.workspace();

    // handle timeouts at the source chain layer

    // add a DeleteLink to the source chain
    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let mut guard = workspace_lock.write().await;
        let workspace: &mut CallZomeWorkspace = &mut guard;
        let source_chain = &mut workspace.source_chain;
        let header_builder = builder::DeleteLink {
            link_add_address,
            base_address,
        };
        let header_hash = source_chain.put(header_builder, None).await?;
        let element = source_chain
            .get_element(&header_hash)?
            .expect("Element we just put in SourceChain must be gettable");
        integrate_to_authored(
            &element,
            workspace.source_chain.elements(),
            &mut workspace.meta_authored,
        )
        .map_err(Box::new)?;
        Ok(DeleteLinkOutput::new(header_hash))
    })
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holo_hash::HeaderHash;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::link::Links;
    use holochain_zome_types::DeleteLinkInput;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_delete_link_add_remove() {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();

        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;

        // links should start empty
        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 0);

        // add a couple of links
        let link_one: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "create_link", ());

        // add a couple of links
        let link_two: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "create_link", ());

        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 2);

        // remove a link
        let _: HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "delete_link",
            DeleteLinkInput::new(link_one)
        );

        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 1);

        // remove a link
        let _: HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "delete_link",
            DeleteLinkInput::new(link_two)
        );

        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 0);

        // Add some links then delete them all
        let _h: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "create_link", ());
        let _h: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "create_link", ());

        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 2);

        crate::call_test_ribosome!(host_access, TestWasm::Link, "delete_all_links", ());

        // Should be no links left
        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 0);
    }
}
