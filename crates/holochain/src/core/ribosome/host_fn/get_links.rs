use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetLinksOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use futures::future::join_all;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_links<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<GetLinksInput>,
) -> Result<Vec<Vec<Link>>, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ read_workspace: Permission::Allow, .. } => {
            let results: Vec<Result<Vec<Link>, _>> = tokio_helper::block_forever_on(async move {
                join_all(inputs.into_iter().map(|input| {
                    async {
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
                        ).dht_get_links(key, GetLinksOptions::default()).await
                    }
                }
                )).await
            });
            let results: Result<Vec<_>, _> = results.into_iter().map(|result|
                match result {
                    Ok(links_vec) => Ok(links_vec),
                    Err(cascade_error) => Err(WasmError::Host(cascade_error.to_string())),
                }
            ).collect();
            Ok(results?)
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use crate::test_utils::conductor_setup::ConductorTestData;
    use crate::test_utils::new_zome_call;
    use crate::test_utils::wait_for_integration_1m;
    use crate::test_utils::WaitOps;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_test_wasm_common::*;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;
    use crate::sweettest::SweetDnaFile;
    use crate::core::ribosome::MockDnaStore;
    use crate::sweettest::SweetConductor;
    use crate::conductor::ConductorBuilder;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_entry_hash_path_children() {
        observability::test_run().ok();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        // ensure foo.bar twice to ensure idempotency
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.bar".to_string()
        ).unwrap();
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.bar".to_string()
        ).unwrap();

        // ensure foo.baz
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.baz".to_string()
        ).unwrap();

        let exists_output: bool = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "exists",
            "foo".to_string()
        ).unwrap();

        assert_eq!(true, exists_output,);

        let foo_bar: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "path_entry_hash",
            "foo.bar".to_string()
        ).unwrap();

        let foo_baz: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "path_entry_hash",
            "foo.baz".to_string()
        ).unwrap();

        let links: Vec<holochain_zome_types::link::Link> = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children",
            "foo".to_string()
        ).unwrap();

        assert_eq!(2, links.len());
        assert_eq!(links[0].target, foo_bar,);
        assert_eq!(links[1].target, foo_baz,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn hash_path_anchor_list_anchors() {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Anchor]).await.unwrap();

        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);
        let bob_pubkey = fixt!(AgentPubKey, Predictable, 1);

        let mut dna_store = MockDnaStore::new();
        dna_store.expect_add_dnas::<Vec<_>>().return_const(());
        dna_store.expect_add_entry_defs::<Vec<_>>().return_const(());
        dna_store.expect_add_dna().return_const(());
        dna_store
            .expect_get()
            .return_const(Some(dna_file.clone().into()));
        dna_store
            .expect_get_entry_def()
            .return_const(EntryDef::default_with_id("thing"));

        let mut conductor =
            SweetConductor::from_builder(ConductorBuilder::with_mock_dna_store(dna_store)).await;

        let apps = conductor
            .setup_app_for_agents(
                "app-",
                &[alice_pubkey.clone(), bob_pubkey.clone()],
                &[dna_file.into()],
            )
            .await
            .unwrap();

        let ((alice,), (bobbo,)) = apps.into_tuples();
        let alice = alice.zome(TestWasm::Anchor);
        let _bobbo = bobbo.zome(TestWasm::Anchor);

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
            vec![239, 154, 133, 224, 208, 133, 175, 213, 70, 185, 115, 244, 123, 202, 246, 45, 142, 171, 111, 225, 134, 232, 124, 124, 51, 166, 3, 131, 56, 27, 76, 231],
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
            vec![81, 42, 172, 138, 250, 246, 182, 118, 113, 130, 93, 133, 115, 98, 166, 208, 53, 223, 29, 45, 135, 67, 32, 146, 234, 249, 5, 8, 19, 179, 179, 16],
        );

        let list_anchor_type_addresses_output: EntryHashes = conductor
            .call(
                &alice,
                "list_anchor_type_addresses",
                ()
            ).await;

        // should be 1 anchor type, "foo"
        assert_eq!(list_anchor_type_addresses_output.0.len(), 1,);
        assert_eq!(
            (list_anchor_type_addresses_output.0)[0]
                .get_raw_32()
                .to_vec(),
            vec![206, 174, 24, 124, 110, 165, 183, 233, 230, 239, 228, 56, 61, 1, 13, 30, 162, 30, 181, 117, 209, 186, 24, 59, 87, 218, 157, 60, 93, 156, 34, 226],
        );

        let list_anchor_addresses_output: EntryHashes = conductor
            .call(
                &alice,
                "list_anchor_addresses",
                "foo".to_string()
            ).await;

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
            .call(
                &alice,
                "list_anchor_tags",
                "foo".to_string()
            )
            .await;

        assert_eq!(
            vec!["bar".to_string(), "baz".to_string()],
            list_anchor_tags_output,
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multi_get_links() {
        observability::test_run().ok();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let _: HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "create_link",
            ()
        ).unwrap();
        let _: HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "create_back_link",
            ()
        ).unwrap();
        let forward_links: Vec<Link> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "get_links",
            ()
        ).unwrap();
        let back_links: Vec<Link> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "get_back_links",
            ()
        ).unwrap();
        let links_bidi: Vec<Vec<Link>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "get_links_bidi",
            ()
        ).unwrap();

        assert_eq!(
            links_bidi,
            vec![forward_links, back_links],
        );

        let forward_link_details: LinkDetails = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "get_link_details",
            ()
        ).unwrap();

        let back_link_details: LinkDetails = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "get_back_link_details",
            ()
        ).unwrap();

        let link_details_bidi: Vec<LinkDetails> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Link,
            "get_link_details_bidi",
            ()
        ).unwrap();
        assert_eq!(
            link_details_bidi,
            vec![forward_link_details, back_link_details],
        );
    }

    #[tokio::test(flavor = "multi_thread")]
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

        wait_for_integration_1m(&alice_call_data.dht_env, expected_count).await;

        let invocation = new_zome_call(
            &alice_call_data.cell_id,
            "get_long_path",
            (),
            TestWasm::Link,
        )
        .unwrap();

        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        let links: Vec<hdk::prelude::Link> = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .decode()
            .unwrap();
        assert_eq!(links.len(), 1);
        conductor_test.shutdown_conductor().await;
    }
}
