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
) -> Result<Vec<Links>, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ read_workspace: Permission::Allow, .. } => {
            let results: Vec<Result<Vec<Link>, _>> = tokio_helper::block_forever_on(async move {
                join_all(inputs.into_iter().map(|input| {
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
                        call_context.host_context.workspace(),
                        call_context.host_context.network().to_owned(),
                    ).into_dht_get_links(key, GetLinksOptions::default())
                }
                )).await
            });
            let results: Result<Vec<_>, _> = results.into_iter().map(|result|
                match result {
                    Ok(links_vec) => Ok(links_vec.into()),
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
    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_test_wasm_common::*;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_entry_hash_path_children() {
        observability::test_run().ok();
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone()).await.unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).await.unwrap();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace;

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
            "hash",
            "foo.bar".to_string()
        ).unwrap();

        let foo_baz: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            "foo.baz".to_string()
        ).unwrap();

        let children_output: holochain_zome_types::link::Links = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children",
            "foo".to_string()
        ).unwrap();

        let links = children_output.into_inner();
        assert_eq!(2, links.len());
        assert_eq!(links[0].target, foo_bar,);
        assert_eq!(links[1].target, foo_baz,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn hash_path_anchor_get_anchor() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone()).await.unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).await.unwrap();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace;

        // anchor foo bar
        let anchor_address_one: EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Anchor,
            "anchor",
            AnchorInput("foo".to_string(), "bar".to_string())
        ).unwrap();

        assert_eq!(
            anchor_address_one.get_raw_32().to_vec(),
            vec![
                25, 68, 104, 205, 38, 19, 71, 158, 115, 188, 249, 175, 248, 71, 83, 86, 126, 131,
                246, 20, 10, 222, 185, 123, 219, 175, 209, 66, 12, 140, 83, 215
            ],
        );

        // anchor foo baz
        let anchor_address_two: EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Anchor,
            "anchor",
            AnchorInput("foo".to_string(), "baz".to_string())
        ).unwrap();

        assert_eq!(
            anchor_address_two.get_raw_32().to_vec(),
            vec![
                130, 158, 169, 16, 161, 104, 109, 116, 108, 147, 130, 214, 150, 32, 57, 52, 27, 39,
                35, 209, 47, 120, 63, 220, 122, 13, 21, 204, 51, 209, 241, 31
            ],
        );

        let get_output: Option<Anchor> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Anchor,
            "get_anchor",
            anchor_address_one
        ).unwrap();

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
        ).unwrap();

        // should be 1 anchor type, "foo"
        assert_eq!(list_anchor_type_addresses_output.0.len(), 1,);
        assert_eq!(
            (list_anchor_type_addresses_output.0)[0]
                .get_raw_32()
                .to_vec(),
            vec![
                36, 198, 140, 31, 125, 166, 8, 15, 167, 149, 247, 118, 206, 134, 173, 221, 96, 215,
                1, 227, 209, 230, 139, 169, 117, 216, 143, 92, 107, 122, 183, 189
            ],
        );

        let list_anchor_addresses_output: EntryHashes = {
            crate::call_test_ribosome!(
                host_access,
                TestWasm::Anchor,
                "list_anchor_addresses",
                "foo".to_string()
            ).unwrap()
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
        ).unwrap();

        assert_eq!(
            vec!["bar".to_string(), "baz".to_string()],
            list_anchor_tags_output,
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

        wait_for_integration_1m(&alice_call_data.env, expected_count).await;

        let invocation = new_zome_call(
            &alice_call_data.cell_id,
            "get_long_path",
            (),
            TestWasm::Link,
        )
        .unwrap();

        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        let links: hdk::prelude::Links = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .decode()
            .unwrap();
        assert_eq!(links.into_inner().len(), 1);
        conductor_test.shutdown_conductor().await;
    }
}
