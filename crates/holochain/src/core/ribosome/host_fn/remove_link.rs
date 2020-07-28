use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::state::source_chain::SourceChainResult;
use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_to_cache;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use holo_hash::HeaderHash;
use holochain_types::element::SignedHeaderHashed;
use holochain_zome_types::header::builder;
use holochain_zome_types::Header;
use holochain_zome_types::RemoveLinkInput;
use holochain_zome_types::RemoveLinkOutput;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn remove_link<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: RemoveLinkInput,
) -> RibosomeResult<RemoveLinkOutput> {
    let link_add_address = input.into_inner();

    // get the base address from the add link header
    // don't allow the wasm developer to get this wrong
    // it is never valid to have divergent base address for add/remove links
    // the subconscious will validate the base address match but we need to fetch it here to
    // include it in the remove link header
    let network = call_context.host_access.network().clone();
    let address = link_add_address.clone();
    let add_link_get_call = |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<Option<SignedHeaderHashed>>> {
        async move {
            let cascade = workspace.cascade(network);
            // @todo use .dht_get() once it supports header hashes
            Ok(cascade.dht_get_header_raw(&address).await?)
        }
        .boxed()
    };
    // handle timeouts at the network layer
    let async_call_context = call_context.clone();
    let maybe_add_link: Option<SignedHeaderHashed> =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            unsafe {
                async_call_context
                    .host_access
                    .workspace()
                    .apply_mut(add_link_get_call)
                    .await
            }
        })??;

    let base_address = match maybe_add_link {
        Some(add_link_signed_header_hash) => {
            match add_link_signed_header_hash.header() {
                Header::LinkAdd(link_add_header) => Ok(link_add_header.base_address.clone()),
                // the add link header hash provided was found but didn't point to an AddLink
                // header (it is something else) so we cannot proceed
                _ => Err(RibosomeError::ElementDeps(link_add_address.clone())),
            }
        }
        // the add link header hash could not be found
        // it's unlikely that a wasm call would have a valid add link header hash from "somewhere"
        // that isn't also discoverable in either the cache or DHT, but it _is_ possible so we have
        // to fail in that case (e.g. the local cache could have GC'd at the same moment the
        // network connection dropped out)
        None => Err(RibosomeError::ElementDeps(link_add_address.clone())),
    }?;

    // add a LinkRemove to the source chain
    let call =
        |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderHash>> {
            async move {
                let source_chain = &mut workspace.source_chain;
                let header_builder = builder::LinkRemove {
                    link_add_address: link_add_address,
                    base_address: base_address,
                };
                let header_hash = source_chain.put(header_builder, None).await?;
                let element = source_chain
                    .get_element(&header_hash)
                    .await?
                    .expect("Element we just put in SourceChain must be gettable");
                integrate_to_cache(
                    &element,
                    &mut workspace.cache_cas,
                    &mut workspace.cache_meta,
                )
                .await
                .map_err(Box::new)?;
                Ok(header_hash)
            }
            .boxed()
        };
    // handle timeouts at the source chain layer
    let header_address =
        tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        }))???;

    Ok(RemoveLinkOutput::new(header_address))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {

    use crate::core::state::workspace::Workspace;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holo_hash::HeaderHash;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::link::Links;
    use holochain_zome_types::RemoveLinkInput;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_remove_link_add_remove() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        let reader = env_ref.reader().unwrap();
        let mut workspace = crate::core::workflow::CallZomeWorkspace::new(&reader, &dbs).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;

        // links should start empty
        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 0);

        // add a couple of links
        let link_one: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "link_entries", ());

        // add a couple of links
        let link_two: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Link, "link_entries", ());

        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 2);

        // remove a link
        let _: HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "remove_link",
            RemoveLinkInput::new(link_one)
        );

        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 1);

        // remove a link
        let _: HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "remove_link",
            RemoveLinkInput::new(link_two)
        );

        let links: Links = crate::call_test_ribosome!(host_access, TestWasm::Link, "get_links", ());

        assert!(links.into_inner().len() == 0);
    }
}
