use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::error::CascadeResult;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn delete_link<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: DeleteLinkInput,
) -> Result<ActionHash, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let DeleteLinkInput {
                address,
                chain_top_ordering,
            } = input;
            // get the base address from the add link action
            // don't allow the wasm developer to get this wrong
            // it is never valid to have divergent base address for add/remove links
            // the subconscious will validate the base address match but we need to fetch it here to
            // include it in the remove link action
            let network = call_context.host_context.network().clone();
            let call_context_2 = call_context.clone();

            // handle timeouts at the network layer
            let address_2 = address.clone();
            let maybe_add_link: Option<SignedActionHashed> =
                tokio_helper::block_forever_on(async move {
                    let workspace = call_context_2.host_context.workspace();
                    CascadeResult::Ok(
                        Cascade::from_workspace_network(&workspace, network)
                            .dht_get(address_2.into(), GetOptions::content())
                            .await?
                            .map(|el| el.into_inner().0),
                    )
                })
                .map_err(|cascade_error| -> RuntimeError {
                    wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                })?;

            let base_address = match maybe_add_link {
                Some(add_link_signed_action_hash) => {
                    match add_link_signed_action_hash.action() {
                        Action::CreateLink(link_add_action) => {
                            Ok(link_add_action.base_address.clone())
                        }
                        // the add link action hash provided was found but didn't point to an AddLink
                        // action (it is something else) so we cannot proceed
                        _ => Err(RibosomeError::RecordDeps(address.clone().into())),
                    }
                }
                // the add link action hash could not be found
                // it's unlikely that a wasm call would have a valid add link action hash from "somewhere"
                // that isn't also discoverable in either the cache or DHT, but it _is_ possible so we have
                // to fail in that case (e.g. the local cache could have GC'd at the same moment the
                // network connection dropped out)
                None => Err(RibosomeError::RecordDeps(address.clone().into())),
            }
            .map_err(|ribosome_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(ribosome_error.to_string())).into()
            })?;

            let source_chain = call_context
                .host_context
                .workspace_write()
                .source_chain()
                .as_ref()
                .expect("Must have source chain if write_workspace access is given");

            // handle timeouts at the source chain layer

            // add a DeleteLink to the source chain
            tokio_helper::block_forever_on(async move {
                let action_builder = builder::DeleteLink {
                    link_add_address: address,
                    base_address,
                };
                let action_hash = source_chain
                    .put(action_builder, None, chain_top_ordering)
                    .await
                    .map_err(|source_chain_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(source_chain_error.to_string())).into()
                    })?;
                Ok(action_hash)
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "delete_link".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;
    use holo_hash::ActionHash;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_delete_link_add_remove() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        // links should start empty
        let links: Vec<Vec<Link>> = conductor.call(&alice, "get_links", ()).await;

        assert!(links.len() == 0);

        // add a couple of links
        let mut link_actions: Vec<ActionHash> = Vec::new();
        for _ in 0..2 {
            link_actions.push(conductor.call(&alice, "create_link", ()).await)
        }

        let links: Vec<Link> = conductor.call(&alice, "get_links", ()).await;

        assert!(links.len() == 2);

        // remove a link
        let _: ActionHash = conductor
            .call(&alice, "delete_link", link_actions[0].clone())
            .await;

        let links: Vec<Link> = conductor.call(&alice, "get_links", ()).await;

        assert!(links.len() == 1);

        // remove a link
        let _: ActionHash = conductor
            .call(&alice, "delete_link", link_actions[1].clone())
            .await;

        let links: Vec<Link> = conductor.call(&alice, "get_links", ()).await;

        assert!(links.len() == 0);

        // Add some links then delete them all
        let _h: ActionHash = conductor.call(&alice, "create_link", ()).await;
        let _h: ActionHash = conductor.call(&alice, "create_link", ()).await;

        let links: Vec<Link> = conductor.call(&alice, "get_links", ()).await;

        assert!(links.len() == 2);

        let _: () = conductor.call(&alice, "delete_all_links", ()).await;

        // Should be no links left
        let links: Vec<Link> = conductor.call(&alice, "get_links", ()).await;

        assert!(links.len() == 0);
    }
}
