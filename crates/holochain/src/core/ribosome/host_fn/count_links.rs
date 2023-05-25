use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

/// Count links
#[allow(clippy::extra_unused_lifetimes)]
#[tracing::instrument(skip(_ribosome, call_context), fields(? call_context.zome, function = ? call_context.function_name))]
pub fn count_links<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    query: LinkQuery,
) -> Result<usize, RuntimeError> {
    tracing::debug!(msg = "Counting links", ?query);
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => {
            tokio_helper::block_forever_on(async move {
                let wire_query = WireLinkQuery {
                    base: query.base,
                    link_type: query.link_type,
                    tag_prefix: query.tag_prefix,
                    before: query.before,
                    after: query.after,
                    author: query.author,
                };

                Cascade::from_workspace_and_network(
                    &call_context.host_context.workspace(),
                    call_context.host_context.network().to_owned(),
                )
                    .dht_count_links(wire_query)
                    .await.map_err(|cascade_error| {
                    wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                })
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "count_links".into(),
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
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn count_links() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, bob, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        // Create a link for Alice
        let _: ActionHash = conductor
            .call(&alice, "create_link", ())
            .await;

        let base: AnyLinkableHash = conductor.call(&alice, "get_count_base", ()).await;

        let count: usize = conductor.call(&alice, "get_count", LinkQuery::new(base.clone(), LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]))).await;
        assert_eq!(1, count);

        // Create a link for Bob
        let _: ActionHash = conductor
            .call(&bob, "create_link", ())
            .await;

        // Check that Alice can see her link and Bob's
        let count: usize = conductor.call(&alice, "get_count", LinkQuery::new(base, LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]))).await;
        assert_eq!(2, count);
    }
}
