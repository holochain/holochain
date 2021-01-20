use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_p2p::actor::GetLinksOptions;
use holochain_state::metadata::LinkMetaKey;
use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_links<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetLinksInput,
) -> RibosomeResult<Links> {
    let GetLinksInput { base_address, tag_prefix } = input;

    // Get zome id
    let zome_id = ribosome.zome_to_id(&call_context.zome)?;

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        // Create the key
        let key = match tag_prefix.as_ref() {
            Some(tag_prefix) => LinkMetaKey::BaseZomeTag(&base_address, zome_id, tag_prefix),
            None => LinkMetaKey::BaseZome(&base_address, zome_id),
        };

        // Get the links from the dht
        let links = call_context
            .host_access
            .workspace()
            .write()
            .await
            .cascade(network)
            .dht_get_links(&key, GetLinksOptions::default())
            .await?;

        Ok(links.into())
    })
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use crate::test_utils::conductor_setup::ConductorTestData;
    use crate::test_utils::new_zome_call;
    use crate::test_utils::wait_for_integration_10s;
    use crate::test_utils::WaitOps;
    use ::fixt::prelude::*;
    use hdk3::prelude::*;
    use holochain_test_wasm_common::*;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_entry_hash_path_children() {
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

        // ensure foo.bar twice to ensure idempotency
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.bar".to_string()
        );
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.bar".to_string()
        );

        // ensure foo.baz
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.baz".to_string()
        );

        let exists_output: bool = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "exists",
            "foo".to_string()
        );

        assert_eq!(true, exists_output,);

        let foo_bar: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            "foo.bar".to_string()
        );

        let foo_baz: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            "foo.baz".to_string()
        );

        let children_output: holochain_zome_types::link::Links = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children",
            "foo".to_string()
        );

        let links = children_output.into_inner();
        assert_eq!(2, links.len());
        assert_eq!(links[0].target, foo_bar,);
        assert_eq!(links[1].target, foo_baz,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn hash_path_anchor_get_anchor() {
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

        // anchor foo bar
        let anchor_address_one: EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Anchor,
            "anchor",
            AnchorInput("foo".to_string(), "bar".to_string())
        );

        assert_eq!(
            anchor_address_one.get_raw_32().to_vec(),
            vec![
                138, 240, 209, 89, 206, 160, 42, 131, 107, 63, 111, 243, 67, 8, 24, 48, 151, 62,
                108, 99, 102, 109, 57, 253, 219, 26, 255, 164, 83, 134, 245, 254
            ],
        );

        // anchor foo baz
        let anchor_address_two: EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Anchor,
            "anchor",
            AnchorInput("foo".to_string(), "baz".to_string())
        );

        assert_eq!(
            anchor_address_two.get_raw_32().to_vec(),
            vec![
                175, 176, 111, 101, 56, 12, 198, 140, 48, 157, 209, 87, 118, 124, 157, 94, 234,
                232, 82, 136, 228, 219, 237, 221, 195, 225, 98, 177, 76, 26, 126, 6
            ],
        );

        let get_output: Option<Anchor> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Anchor,
            "get_anchor",
            anchor_address_one
        );

        assert_eq!(
            Some(Anchor {
                anchor_type: "foo".into(),
                anchor_text: Some("bar".into()),
            }),
            get_output,
        );

        let list_anchor_type_addresses_output: EntryHashes = crate::call_test_ribosome!(
            host_access,
            TestWasm::Anchor,
            "list_anchor_type_addresses",
            ()
        );

        // should be 1 anchor type, "foo"
        assert_eq!(list_anchor_type_addresses_output.0.len(), 1,);
        assert_eq!(
            (list_anchor_type_addresses_output.0)[0]
                .get_raw_32()
                .to_vec(),
            vec![
                14, 28, 21, 33, 162, 54, 200, 39, 170, 131, 53, 252, 229, 108, 231, 41, 38, 79, 4,
                232, 36, 95, 237, 120, 101, 249, 248, 91, 140, 51, 61, 124
            ],
        );

        let list_anchor_addresses_output: EntryHashes = {
            crate::call_test_ribosome!(
                host_access,
                TestWasm::Anchor,
                "list_anchor_addresses",
                "foo".to_string()
            )
        };

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

        let list_anchor_tags_output: Vec<String> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Anchor,
            "list_anchor_tags",
            "foo".to_string()
        );

        assert_eq!(
            vec!["bar".to_string(), "baz".to_string()],
            list_anchor_tags_output,
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn dup_path_test() {
        observability::test_run().ok();
        let zomes = vec![TestWasm::Link];
        let mut conductor_test = ConductorTestData::two_agents(zomes, false).await;
        let handle = conductor_test.handle();
        let alice_call_data = &conductor_test.alice_call_data();

        let invocation = new_zome_call(
            &alice_call_data.cell_id,
            "commit_existing_path",
            (),
            TestWasm::Link,
        )
        .unwrap();
        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        assert_matches!(result, ZomeCallResponse::Ok(_));
        let invocation = new_zome_call(
            &alice_call_data.cell_id,
            "commit_existing_path",
            (),
            TestWasm::Link,
        )
        .unwrap();
        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        assert_matches!(result, ZomeCallResponse::Ok(_));

        let mut expected_count = WaitOps::start() + WaitOps::path(1);
        // Plus one length path for the commit existing.
        expected_count += WaitOps::ENTRY + WaitOps::LINK;

        wait_for_integration_10s(&alice_call_data.env, expected_count).await;

        let invocation = new_zome_call(
            &alice_call_data.cell_id,
            "get_long_path",
            (),
            TestWasm::Link,
        )
        .unwrap();

        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        let links: hdk3::prelude::Links = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .decode()
            .unwrap();
        assert_eq!(links.into_inner().len(), 1);
        conductor_test.shutdown_conductor().await;
    }
}
