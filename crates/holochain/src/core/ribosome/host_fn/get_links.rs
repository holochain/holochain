use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use futures::StreamExt;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetLinksOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
#[tracing::instrument(skip(_ribosome, call_context), fields(?call_context.zome, function = ?call_context.function_name))]
pub fn get_links<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<GetLinksInput>,
) -> Result<Vec<Vec<Link>>, RuntimeError> {
    let num_requests = inputs.len();
    tracing::debug!("Starting with {} requests.", num_requests);
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => {
            let results: Vec<Result<Vec<Link>, RibosomeError>> =
                tokio_helper::block_forever_on(async move {
                    futures::stream::iter(inputs.into_iter().map(|input| async {
                        let GetLinksInput {
                            base_address,
                            link_type,
                            tag_prefix,
                        } = input;

                        let key = WireLinkKey {
                            base: base_address,
                            type_query: Some(link_type),
                            tag: tag_prefix,
                        };
                        Ok(Cascade::from_workspace_network(
                            &call_context.host_context.workspace(),
                            call_context.host_context.network().to_owned(),
                        )
                        .dht_get_links(key, GetLinksOptions::default())
                        .await?)
                    }))
                    // Limit concurrent calls to 10 as each call
                    // can spawn multiple connections.
                    .buffered(10)
                    .collect()
                    .await
                });
            let results: Result<Vec<_>, RuntimeError> = results
                .into_iter()
                .map(|result| match result {
                    Ok(links_vec) => Ok(links_vec),
                    Err(cascade_error) => {
                        Err(wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into())
                    }
                })
                .collect();
            let results = results?;
            tracing::debug!(
                "Ending with {} out of {} results, {} total links and {} total responses.",
                results.iter().filter(|r| !r.is_empty()).count(),
                num_requests,
                results.iter().map(|r| r.len()).sum::<usize>(),
                results.len(),
            );
            Ok(results)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get_links".into(),
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
    use holochain_test_wasm_common::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_entry_hash_path_children() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::HashPath).await;

        // ensure foo.bar twice to ensure idempotency
        for _ in 0..2 {
            let _: () = conductor
                .call(&alice, "ensure", "foo.bar".to_string())
                .await;
        }

        // ensure foo.baz
        let _: () = conductor
            .call(&alice, "ensure", "foo.baz".to_string())
            .await;

        let exists_output: bool = conductor.call(&alice, "exists", "foo".to_string()).await;

        assert!(exists_output);

        let foo_bar: holo_hash::AnyLinkableHash = conductor
            .call(&alice, "path_entry_hash", "foo.bar".to_string())
            .await;

        let foo_baz: holo_hash::AnyLinkableHash = conductor
            .call(&alice, "path_entry_hash", "foo.baz".to_string())
            .await;

        let links: Vec<holochain_zome_types::link::Link> =
            conductor.call(&alice, "children", "foo".to_string()).await;

        assert_eq!(2, links.len());
        assert_eq!(links[0].target, foo_bar);
        assert_eq!(links[1].target, foo_baz);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn hash_path_anchor_list_anchors() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Anchor).await;

        // anchor foo bar
        let anchor_address_one: EntryHash = conductor
            .call(
                &alice,
                "anchor",
                AnchorInput("foo".to_string(), "bar".to_string()),
            )
            .await;

        let expect = Path::from(vec![
            hdk::hash_path::path::Component::new(hdk::hash_path::anchor::ROOT.to_vec()),
            hdk::hash_path::path::Component::from("foo".as_bytes().to_vec()),
            hdk::hash_path::path::Component::from("bar".as_bytes().to_vec()),
        ]);
        assert_eq!(
            anchor_address_one,
            EntryHash::with_data_sync(&Entry::App(AppEntryBytes(expect.try_into().unwrap())))
        );

        // anchor foo baz
        let anchor_address_two: EntryHash = conductor
            .call(
                &alice,
                "anchor",
                AnchorInput("foo".to_string(), "baz".to_string()),
            )
            .await;

        let expect = Path::from(vec![
            hdk::hash_path::path::Component::new(hdk::hash_path::anchor::ROOT.to_vec()),
            hdk::hash_path::path::Component::from("foo".as_bytes().to_vec()),
            hdk::hash_path::path::Component::from("baz".as_bytes().to_vec()),
        ]);
        assert_eq!(
            anchor_address_two,
            EntryHash::with_data_sync(&Entry::App(AppEntryBytes(expect.try_into().unwrap())))
        );

        let list_anchor_type_addresses_output: EntryHashes = conductor
            .call(&alice, "list_anchor_type_addresses", ())
            .await;

        let expect = Path::from(vec![
            hdk::hash_path::path::Component::new(hdk::hash_path::anchor::ROOT.to_vec()),
            hdk::hash_path::path::Component::from("foo".as_bytes().to_vec()),
        ]);
        // should be 1 anchor type, "foo"
        assert_eq!(list_anchor_type_addresses_output.0.len(), 1,);
        assert_eq!(
            (list_anchor_type_addresses_output.0)[0],
            EntryHash::with_data_sync(&Entry::App(AppEntryBytes(expect.try_into().unwrap())))
        );

        let list_anchor_addresses_output: EntryHashes = conductor
            .call(&alice, "list_anchor_addresses", "foo".to_string())
            .await;

        // should be 2 anchors under "foo" sorted by hash
        assert_eq!(list_anchor_addresses_output.0.len(), 2,);
        assert_eq!(
            (list_anchor_addresses_output.0)[0].get_raw_32().to_vec(),
            anchor_address_one.get_raw_32().to_vec(),
        );
        assert_eq!(
            (list_anchor_addresses_output.0)[1].get_raw_32().to_vec(),
            anchor_address_two.get_raw_32().to_vec(),
        );

        let list_anchor_tags_output: Vec<String> = conductor
            .call(&alice, "list_anchor_tags", "foo".to_string())
            .await;

        assert_eq!(
            vec!["bar".to_string(), "baz".to_string()],
            list_anchor_tags_output,
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn baseless_get_links() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        let action_hash: ActionHash = conductor.call(&alice, "create_baseless_link", ()).await;
        let links: Vec<Link> = conductor.call(&alice, "get_baseless_links", ()).await;

        assert_eq!(links[0].create_link_hash, action_hash);
        assert_eq!(
            links[0].target,
            EntryHash::from_raw_36([2_u8; 36].to_vec()).into(),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn external_get_links() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        let action_hash: ActionHash = conductor
            .call(&alice, "create_external_base_link", ())
            .await;
        let links: Vec<Link> = conductor.call(&alice, "get_external_links", ()).await;

        assert_eq!(links[0].create_link_hash, action_hash);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multi_get_links() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        let _: ActionHash = conductor.call(&alice, "create_link", ()).await;
        let _: ActionHash = conductor.call(&alice, "create_back_link", ()).await;
        let forward_links: Vec<Link> = conductor.call(&alice, "get_links", ()).await;
        let back_links: Vec<Link> = conductor.call(&alice, "get_back_links", ()).await;
        let links_bidi: Vec<Vec<Link>> = conductor.call(&alice, "get_links_bidi", ()).await;

        assert_eq!(links_bidi, vec![forward_links, back_links],);

        let forward_link_details: LinkDetails =
            conductor.call(&alice, "get_link_details", ()).await;
        let back_link_details: LinkDetails =
            conductor.call(&alice, "get_back_link_details", ()).await;
        let link_details_bidi: Vec<LinkDetails> =
            conductor.call(&alice, "get_link_details_bidi", ()).await;

        assert_eq!(
            link_details_bidi,
            vec![forward_link_details, back_link_details],
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dup_path_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        for _ in 0..2 {
            let _result: () = conductor.call(&alice, "commit_existing_path", ()).await;
        }

        let links: Vec<hdk::prelude::Link> = conductor.call(&alice, "get_long_path", ()).await;
        assert_eq!(links.len(), 1);
    }
}
