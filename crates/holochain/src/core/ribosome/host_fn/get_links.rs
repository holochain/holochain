use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use futures::future::join_all;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetLinksOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_links<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<GetLinksInput>,
) -> Result<Vec<Vec<Link>>, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => {
            let results: Vec<Result<Vec<Link>, _>> = tokio_helper::block_forever_on(async move {
                join_all(inputs.into_iter().map(|input| async {
                    let GetLinksInput {
                        base_address,
                        tag_prefix,
                    } = input;
                    let zome_id = ribosome
                        .zome_to_id(&call_context.zome)
                        .expect("Failed to get ID for current zome.");
                    let key = WireLinkKey {
                        base: base_address,
                        zome_id,
                        tag: tag_prefix,
                    };
                    Cascade::from_workspace_network(
                        &call_context.host_context.workspace(),
                        call_context.host_context.network().to_owned(),
                    )
                    .dht_get_links(key, GetLinksOptions::default())
                    .await
                }))
                .await
            });
            let results: Result<Vec<_>, _> = results
                .into_iter()
                .map(|result| match result {
                    Ok(links_vec) => Ok(links_vec),
                    Err(cascade_error) => Err(WasmError::Host(cascade_error.to_string())),
                })
                .collect();
            Ok(results?)
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get_links".into(),
            )
            .to_string(),
        )),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::test_utils::wait_for_integration_1m;
    use crate::test_utils::WaitOps;
    use hdk::prelude::*;
    use holochain_test_wasm_common::*;
    use holochain_wasm_test_utils::TestWasm;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;

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

        let foo_bar: holo_hash::EntryHash = conductor
            .call(&alice, "path_entry_hash", "foo.bar".to_string())
            .await;

        let foo_baz: holo_hash::EntryHash = conductor
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

        assert_eq!(
            anchor_address_one.get_raw_32().to_vec(),
            vec![174, 222, 191, 173, 245, 226, 135, 240, 9, 44, 238, 112, 158, 41, 73, 28, 175, 94, 206, 82, 82, 109, 9, 156, 73, 22, 188, 213, 148, 21, 234, 45],
        );

        // anchor foo baz
        let anchor_address_two: EntryHash = conductor
            .call(
                &alice,
                "anchor",
                AnchorInput("foo".to_string(), "baz".to_string()),
            )
            .await;

        assert_eq!(
            anchor_address_two.get_raw_32().to_vec(),
            vec![21, 114, 154, 43, 189, 82, 166, 104, 159, 55, 86, 94, 68, 245, 79, 49, 187, 175, 236, 67, 38, 216, 232, 239, 18, 83, 98, 200, 136, 198, 232, 117],
        );

        let list_anchor_type_addresses_output: EntryHashes = conductor
            .call(&alice, "list_anchor_type_addresses", ())
            .await;

        // should be 1 anchor type, "foo"
        assert_eq!(list_anchor_type_addresses_output.0.len(), 1,);
        assert_eq!(
            (list_anchor_type_addresses_output.0)[0]
                .get_raw_32()
                .to_vec(),
            vec![5, 114, 66, 208, 85, 124, 76, 245, 245, 255, 31, 76, 173, 73, 168, 139, 56, 20, 93, 162, 167, 43, 203, 164, 172, 158, 29, 43, 74, 254, 81, 241],
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
    async fn multi_get_links() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        let _: HeaderHash = conductor.call(&alice, "create_link", ()).await;
        let _: HeaderHash = conductor.call(&alice, "create_back_link", ()).await;
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
            conductor, alice, alice_host_fn_caller, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        for _ in 0..2 {
            let _result: () = conductor.call(&alice, "commit_existing_path", ()).await;
        }

        let mut expected_count = WaitOps::start() + WaitOps::path(1);
        // Plus one length path for the commit existing.
        expected_count += WaitOps::ENTRY + WaitOps::LINK;

        wait_for_integration_1m(&alice_host_fn_caller.dht_db, expected_count).await;

        let links: Vec<hdk::prelude::Link> = conductor.call(&alice, "get_long_path", ()).await;
        assert_eq!(links.len(), 1);
    }
}
