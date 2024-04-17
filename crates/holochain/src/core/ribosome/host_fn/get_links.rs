use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use futures::StreamExt;
use holochain_cascade::CascadeImpl;
use holochain_p2p::actor::GetLinksOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

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
                    let call_context_iter = std::iter::from_fn(|| Some(call_context.clone()));
                    futures::stream::iter(
                        std::iter::zip(inputs.into_iter(), call_context_iter).map(
                            |(input, call_context)| async move {
                                let GetLinksInput {
                                    base_address,
                                    link_type,
                                    get_options,
                                    tag_prefix,
                                    after,
                                    before,
                                    author,
                                } = input;

                                let key = WireLinkKey {
                                    base: base_address,
                                    type_query: link_type,
                                    tag: tag_prefix,
                                    after,
                                    before,
                                    author,
                                };
                                Ok(CascadeImpl::from_workspace_and_network(
                                    &call_context.host_context.workspace(),
                                    call_context.host_context.network().to_owned(),
                                )
                                .dht_get_links(
                                    key,
                                    GetLinksOptions {
                                        get_options,
                                        ..Default::default()
                                    },
                                )
                                .await?)
                            },
                        ),
                    )
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
    use crate::{
        core::ribosome::wasm_test::RibosomeTestFixture,
        sweettest::{SweetConductorBatch, SweetConductorConfig, SweetDnaFile},
    };
    use hdk::prelude::*;
    use holochain_test_wasm_common::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_entry_hash_path_children() {
        holochain_trace::test_run();
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
        holochain_trace::test_run();
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
            hdk::prelude::Component::new(hdi::hash_path::anchor::ROOT.to_vec()),
            hdk::prelude::Component::from("foo".as_bytes().to_vec()),
            hdk::prelude::Component::from("bar".as_bytes().to_vec()),
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
            hdk::prelude::Component::new(hdi::hash_path::anchor::ROOT.to_vec()),
            hdk::prelude::Component::from("foo".as_bytes().to_vec()),
            hdk::prelude::Component::from("baz".as_bytes().to_vec()),
        ]);
        assert_eq!(
            anchor_address_two,
            EntryHash::with_data_sync(&Entry::App(AppEntryBytes(expect.try_into().unwrap())))
        );

        let list_anchor_type_addresses_output: EntryHashes = conductor
            .call(&alice, "list_anchor_type_addresses", ())
            .await;

        let expect = Path::from(vec![
            hdk::prelude::Component::new(hdi::hash_path::anchor::ROOT.to_vec()),
            hdk::prelude::Component::from("foo".as_bytes().to_vec()),
        ]);
        // should be 1 anchor type, "foo"
        assert_eq!(list_anchor_type_addresses_output.0.len(), 1);
        assert_eq!(
            (list_anchor_type_addresses_output.0)[0],
            EntryHash::with_data_sync(&Entry::App(AppEntryBytes(expect.try_into().unwrap())))
        );

        let list_anchor_addresses_output: EntryHashes = conductor
            .call(&alice, "list_anchor_addresses", "foo".to_string())
            .await;

        // should be 2 anchors under "foo" sorted by hash
        assert_eq!(list_anchor_addresses_output.0.len(), 2);
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
        holochain_trace::test_run();
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
        holochain_trace::test_run();
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
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        let t1: Timestamp = conductor.call(&alice, "get_time", ()).await;
        let _: ActionHash = conductor.call(&alice, "create_link", ()).await;
        let t2: Timestamp = conductor.call(&alice, "get_time", ()).await;
        let _: ActionHash = conductor.call(&alice, "create_back_link", ()).await;
        let t3: Timestamp = conductor.call(&alice, "get_time", ()).await;
        let forward_links: Vec<Link> = conductor.call(&alice, "get_links", ()).await;
        let back_links: Vec<Link> = conductor.call(&alice, "get_back_links", ()).await;
        let links_bidi: Vec<Vec<Link>> = conductor.call(&alice, "get_links_bidi", ()).await;
        let hash_path_a: holo_hash::AnyLinkableHash =
            conductor.call(&alice, "get_path_hash", "a").await;
        let hash_path_b: holo_hash::AnyLinkableHash =
            conductor.call(&alice, "get_path_hash", "b").await;

        let forward_link_0 = forward_links.get(0).unwrap();
        assert_eq!(forward_link_0.base, hash_path_a);
        assert_eq!(forward_link_0.target, hash_path_b);
        assert_eq!(
            forward_link_0.author,
            alice.cell_id().agent_pubkey().clone()
        );
        assert_eq!(forward_link_0.tag, LinkTag::from(()));
        assert_eq!(forward_link_0.link_type, LinkType(0));
        assert_eq!(forward_link_0.zome_index, ZomeIndex(0));
        assert!(t1 <= forward_link_0.timestamp && t2 >= forward_link_0.timestamp);

        let back_link_0 = back_links.get(0).unwrap();
        assert_eq!(back_link_0.base, hash_path_b);
        assert_eq!(back_link_0.target, hash_path_a);
        assert_eq!(back_link_0.author, alice.cell_id().agent_pubkey().clone());
        assert_eq!(back_link_0.tag, LinkTag::from(()));
        assert_eq!(back_link_0.link_type, LinkType(0));
        assert_eq!(back_link_0.zome_index, ZomeIndex(0));
        assert!(t2 <= back_link_0.timestamp && t3 >= back_link_0.timestamp);
        assert_eq!(links_bidi, vec![forward_links.clone(), back_links.clone()]);

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
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        for _ in 0..2 {
            let _result: () = conductor.call(&alice, "commit_existing_path", ()).await;
        }

        let links: Vec<hdk::prelude::Link> = conductor.call(&alice, "get_long_path", ()).await;
        assert_eq!(links.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_links_filtered_by_tag_prefix() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor,
            alice,
            bob,
            ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        let hash_a: ActionHash = conductor
            .call(&alice, "create_tagged_link", "a".to_string())
            .await;

        let hash_a_b: ActionHash = conductor
            .call(&bob, "create_tagged_link", "a.b".to_string())
            .await;

        let hash_a_b_c: ActionHash = conductor
            .call(&bob, "create_tagged_link", "a.b.c".to_string())
            .await;

        let hash_b: ActionHash = conductor
            .call(&alice, "create_tagged_link", "b".to_string())
            .await;

        let hash_b_a: ActionHash = conductor
            .call(&bob, "create_tagged_link", "b.a".to_string())
            .await;

        // Get the base all the links are attached from
        let base: AnyLinkableHash = conductor.call(&alice, "get_base_hash", ()).await;

        // Get all the links to check they've been created as expected
        let links: Vec<Link> = conductor
            .call(
                &alice,
                "get_links_with_query",
                GetLinksInputBuilder::try_new(
                    base.clone(),
                    LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]),
                )
                .unwrap()
                .build(),
            )
            .await;
        assert_eq!(5, links.len());

        let links: Vec<Link> = conductor
            .call(
                &alice,
                "get_links_with_query",
                GetLinksInputBuilder::try_new(
                    base.clone(),
                    LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]),
                )
                .unwrap()
                .tag_prefix(LinkTag::new("a"))
                .build(),
            )
            .await;
        assert_eq!(
            vec![hash_a.clone(), hash_a_b.clone(), hash_a_b_c.clone()],
            links
                .into_iter()
                .map(|l| l.create_link_hash)
                .collect::<Vec<ActionHash>>()
        );

        let links: Vec<Link> = conductor
            .call(
                &alice,
                "get_links_with_query",
                GetLinksInputBuilder::try_new(
                    base.clone(),
                    LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]),
                )
                .unwrap()
                .tag_prefix(LinkTag::new("a.b"))
                .build(),
            )
            .await;
        assert_eq!(
            vec![hash_a_b, hash_a_b_c],
            links
                .into_iter()
                .map(|l| l.create_link_hash)
                .collect::<Vec<ActionHash>>()
        );

        let links: Vec<Link> = conductor
            .call(
                &alice,
                "get_links_with_query",
                GetLinksInputBuilder::try_new(
                    base.clone(),
                    LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]),
                )
                .unwrap()
                .tag_prefix(LinkTag::new("b"))
                .build(),
            )
            .await;
        assert_eq!(
            vec![hash_b, hash_b_a],
            links
                .into_iter()
                .map(|l| l.create_link_hash)
                .collect::<Vec<ActionHash>>()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_links_filtered_by_timestamp_and_author() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor,
            alice,
            bob,
            ..
        } = RibosomeTestFixture::new(TestWasm::Link).await;

        let hash_a: ActionHash = conductor
            .call(&alice, "create_tagged_link", "a".to_string())
            .await;

        let hash_b: ActionHash = conductor
            .call(&bob, "create_tagged_link", "b".to_string())
            .await;

        let mid_time = Timestamp::now();

        let hash_c: ActionHash = conductor
            .call(&alice, "create_tagged_link", "c".to_string())
            .await;

        let hash_d: ActionHash = conductor
            .call(&bob, "create_tagged_link", "d".to_string())
            .await;

        // Get the base all the links are attached from
        let base: AnyLinkableHash = conductor.call(&alice, "get_base_hash", ()).await;

        // Get all the links to check they've been created as expected
        let links: Vec<Link> = conductor
            .call(
                &alice,
                "get_links_with_query",
                GetLinksInputBuilder::try_new(
                    base.clone(),
                    LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]),
                )
                .unwrap()
                .build(),
            )
            .await;
        assert_eq!(4, links.len());

        // Filter by created before
        let links: Vec<Link> = conductor
            .call(
                &alice,
                "get_links_with_query",
                GetLinksInputBuilder::try_new(
                    base.clone(),
                    LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]),
                )
                .unwrap()
                .before(mid_time)
                .build(),
            )
            .await;
        assert_eq!(
            vec![hash_a.clone(), hash_b],
            links
                .into_iter()
                .map(|l| l.create_link_hash)
                .collect::<Vec<ActionHash>>()
        );

        // Filter by created after
        let links: Vec<Link> = conductor
            .call(
                &alice,
                "get_links_with_query",
                GetLinksInputBuilder::try_new(
                    base.clone(),
                    LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]),
                )
                .unwrap()
                .after(mid_time)
                .build(),
            )
            .await;
        assert_eq!(
            vec![hash_c.clone(), hash_d],
            links
                .into_iter()
                .map(|l| l.create_link_hash)
                .collect::<Vec<ActionHash>>()
        );

        // Filter by author
        let links: Vec<Link> = conductor
            .call(
                &alice,
                "get_links_with_query",
                GetLinksInputBuilder::try_new(
                    base,
                    LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]),
                )
                .unwrap()
                .author(alice.cell_id().agent_pubkey().clone())
                .build(),
            )
            .await;
        assert_eq!(
            vec![hash_a, hash_c],
            links
                .into_iter()
                .map(|l| l.create_link_hash)
                .collect::<Vec<ActionHash>>()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_links_local_only() {
        holochain_trace::test_run();
        // agents should not pass around data
        let config = SweetConductorConfig::rendezvous(false).no_dpki().tune(|config| {
            config.disable_historical_gossip = true;
            config.disable_recent_gossip = true;
            config.disable_publish = true;
        });
        let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;
        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Link]).await;
        let apps = conductors.setup_app("test", &[dna_file]).await.unwrap();

        // alice creates a link
        let zome_alice = apps[0].cells()[0].zome(TestWasm::Link.coordinator_zome_name());
        let _: ActionHash = conductors[0].call(&zome_alice, "create_link", ()).await;

        // now make both agents aware of each other
        conductors.exchange_peer_info().await;

        // bob gets links locally only
        let zome_bob = apps[1].cells()[0].zome(TestWasm::Link.coordinator_zome_name());
        let local_links: Vec<Link> = conductors[1]
            .call(&zome_bob, "get_links_local_only", ())
            .await;
        // links should be empty
        assert_eq!(local_links.len(), 0);
    }
}
