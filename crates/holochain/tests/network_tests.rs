#![cfg(feature = "test_utils")]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(deprecated)]
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use futures::future::Either;
use futures::future::FutureExt;
use ghost_actor::GhostControlSender;
use hdk::prelude::EntryVisibility;
use holo_hash::hash_type;
use holo_hash::hash_type::AnyDht;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
use holochain::conductor::interface::websocket::test_utils::setup_app;
use holochain::core::workflow::produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertResult;
use holochain::core::workflow::CallZomeWorkspace;
use holochain::test_utils::test_network;
use holochain_cascade::integrate_single_metadata;
use holochain_p2p::actor::GetLinksOptions;
use holochain_p2p::actor::GetMetaOptions;
use holochain_p2p::HolochainP2pCell;
use holochain_p2p::HolochainP2pRef;
use holochain_serialized_bytes::SerializedBytes;
use holochain_sqlite::db::DbWrite;
use holochain_sqlite::db::ReadManager;
use holochain_sqlite::prelude::BufferedStore;
use holochain_sqlite::prelude::IntegratedPrefix;
use holochain_sqlite::prelude::WriteManager;
use holochain_sqlite::test_utils::test_cell_env;
use holochain_state::element_buf::ElementBuf;
use holochain_state::metadata::MetadataBuf;
use holochain_state::metadata::MetadataBufT;
use holochain_types::prelude::*;
use holochain_types::prelude::*;

use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::Entry;
use holochain_zome_types::HeaderHashed;
use maplit::btreeset;
use observability;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::convert::TryInto;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::*;
use unwrap_to::unwrap_to;

use holochain::test_utils::host_fn_caller::*;

/*
#[tokio::test(threaded_scheduler)]
#[ignore = "flaky"]
async fn get_updates_cache() {
    observability::test_run().ok();
    // Database setup
    let test_env = test_cell_env();
    let env = test_env.env();

    let (element_fixt_store, _) = generate_fixt_store().await;
    let expected = element_fixt_store
        .iter()
        .next()
        .map(|(h, e)| (h.clone(), e.clone()))
        .unwrap();

    // Create the cascade
    let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();
    let (network, shutdown) = run_fixt_network(element_fixt_store, BTreeMap::new()).await;

    {
        // Construct the cascade with a network
        let mut cascade = workspace.cascade(network);

        // Call fetch element
        cascade
            .fetch_element_via_header(expected.0.clone().into(), Default::default())
            .await
            .unwrap();
    }

    // Check the cache has been updated
    let result = workspace
        .element_cache
        .get_element(&expected.0)
        .unwrap()
        .unwrap();
    assert_eq!(result.header(), expected.1.header());
    assert_eq!(result.entry(), expected.1.entry());

    shutdown.clean().await;
}
*/

/*
#[tokio::test(threaded_scheduler)]
#[ignore = "flaky!"]
async fn get_meta_updates_meta_cache() {
    observability::test_run().ok();
    // Database setup
    let test_env = test_cell_env();
    let env = test_env.env();
    let env_ref = env.guard();

    // Setup other metadata store with fixtures attached
    // to known entry hash
    let (_, meta_fixt_store) = generate_fixt_store().await;
    let expected = meta_fixt_store
        .iter()
        .next()
        .map(|(h, e)| (h.clone(), e.clone()))
        .unwrap();

    // Create the cascade
    let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();
    let (network, shutdown) = run_fixt_network(BTreeMap::new(), meta_fixt_store).await;

    let returned = {
        // Construct the cascade with a network
        let mut cascade = workspace.cascade(network);

        // Create GetMetaOptions
        let options = GetMetaOptions::default();

        // Call fetch element
        cascade
            .fetch_meta(expected.0.clone().into(), options)
            .await
            .unwrap()
            .first()
            .cloned()
            .unwrap()
    };

    // Check the returned element is correct
    assert_eq!(returned.headers.len(), 1);
    assert_eq!(returned.headers.into_iter().next().unwrap(), expected.1);
    let result = {
        let reader = env_ref.reader().unwrap();

        // Check the cache has been updated
        workspace
            .meta_cache
            .get_headers(
                &reader,
                match expected.0.hash_type().clone() {
                    hash_type::AnyDht::Entry => expected.0.clone().into(),
                    _ => unreachable!(),
                },
            )
            .unwrap()
            .collect::<Vec<_>>()
            .unwrap()
    };
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], expected.1);

    shutdown.clean().await;
}
*/

#[tokio::test(threaded_scheduler)]
#[ignore = "flaky"]
async fn get_from_another_agent() {
    observability::test_run().ok();
    let dna_file = DnaFile::new(
        DnaDef {
            name: "dht_get_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::Create.into()].into(),
        },
        vec![TestWasm::Create.into()],
    )
    .await
    .unwrap();

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
    let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

    let bob_agent_id = fake_agent_pubkey_2();
    let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
    let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());

    let mut dna_store = MockDnaStore::new();

    dna_store.expect_get().return_const(Some(dna_file.clone()));
    dna_store
        .expect_add_dnas::<Vec<_>>()
        .times(2)
        .return_const(());
    dna_store
        .expect_add_entry_defs::<Vec<_>>()
        .times(2)
        .return_const(());
    dna_store.expect_get_entry_def().return_const(None);

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        dna_store,
    )
    .await;

    let options = GetOptions::latest();

    // Bob store element
    let entry = Post("Bananas are good for you".into());
    let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
    let header_hash = {
        let call_data = HostFnCaller::create(&bob_cell_id, &handle, &dna_file).await;
        let header_hash = call_data
            .commit_entry(entry.clone().try_into().unwrap(), POST_ID)
            .await;

        // Bob is not an authority yet
        // Make Bob an "authority"
        fake_authority(header_hash.clone().into(), &call_data).await;
        header_hash
    };

    // Alice get element from bob
    let element = {
        let call_data = HostFnCaller::create(&alice_cell_id, &handle, &dna_file).await;
        call_data
            .get(entry_hash.clone().into(), options.clone())
            .await
    };

    let (signed_header, ret_entry) = element.unwrap().into_inner();

    // TODO: Check signed header is the same header

    // Check Bob is the author
    assert_eq!(*signed_header.header().author(), bob_agent_id);

    // Check entry is the same
    let ret_entry: Post = ret_entry.into_option().unwrap().try_into().unwrap();
    assert_eq!(entry, ret_entry);

    let new_entry = Post("Bananas are bendy".into());
    let (remove_hash, update_hash) = {
        let call_data = HostFnCaller::create(&bob_cell_id, &handle, &dna_file).await;
        let remove_hash = call_data.delete_entry(header_hash.clone()).await;

        fake_authority(remove_hash.clone().into(), &call_data).await;
        let update_hash = call_data
            .update_entry(
                new_entry.clone().try_into().unwrap(),
                POST_ID,
                header_hash.clone(),
            )
            .await;
        fake_authority(update_hash.clone().into(), &call_data).await;
        (remove_hash, update_hash)
    };

    // Alice get element from bob
    let (entry_details, header_details) = {
        let call_data = HostFnCaller::create(&alice_cell_id, &handle, &dna_file).await;
        debug!(the_entry_hash = ?entry_hash);
        let entry_details = call_data
            .get_details(entry_hash.into(), options.clone())
            .await
            .unwrap();
        let header_details = call_data
            .get_details(header_hash.clone().into(), options.clone())
            .await
            .unwrap();
        (entry_details, header_details)
    };

    let entry_details = unwrap_to!(entry_details => Details::Entry).clone();
    let header_details = unwrap_to!(header_details => Details::Element).clone();

    assert_eq!(Post::try_from(entry_details.entry).unwrap(), entry);
    assert_eq!(entry_details.headers.len(), 1);
    assert_eq!(entry_details.deletes.len(), 1);
    assert_eq!(entry_details.updates.len(), 1);
    assert_eq!(entry_details.entry_dht_status, EntryDhtStatus::Dead);
    assert_eq!(
        *entry_details.headers.get(0).unwrap().header_address(),
        header_hash
    );
    assert_eq!(
        *entry_details.deletes.get(0).unwrap().header_address(),
        remove_hash
    );
    assert_eq!(
        *entry_details.updates.get(0).unwrap().header_address(),
        update_hash
    );

    assert_eq!(header_details.deletes.len(), 1);
    assert_eq!(*header_details.element.header_address(), header_hash);
    assert_eq!(
        *entry_details.deletes.get(0).unwrap().header_address(),
        remove_hash
    );

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

#[tokio::test(threaded_scheduler)]
#[ignore = "flaky for some reason"]
async fn get_links_from_another_agent() {
    observability::test_run().ok();
    let dna_file = DnaFile::new(
        DnaDef {
            name: "dht_get_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::Create.into()].into(),
        },
        vec![TestWasm::Create.into()],
    )
    .await
    .unwrap();

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
    let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

    let bob_agent_id = fake_agent_pubkey_2();
    let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
    let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());

    let mut dna_store = MockDnaStore::new();

    dna_store.expect_get().return_const(Some(dna_file.clone()));
    dna_store
        .expect_add_dnas::<Vec<_>>()
        .times(2)
        .return_const(());
    dna_store
        .expect_add_entry_defs::<Vec<_>>()
        .times(2)
        .return_const(());
    dna_store.expect_get_entry_def().return_const(None);

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        dna_store,
    )
    .await;

    let link_options = GetLinksOptions { timeout_ms: None };

    // Bob store links
    let base = Post("Bananas are good for you".into());
    let target = Post("Potassium is radioactive".into());
    let base_entry_hash = EntryHash::with_data_sync(&Entry::try_from(base.clone()).unwrap());
    let target_entry_hash = EntryHash::with_data_sync(&Entry::try_from(target.clone()).unwrap());
    let link_tag = fixt!(LinkTag);
    let link_add_hash = {
        let call_data = HostFnCaller::create(&bob_cell_id, &handle, &dna_file).await;
        let base_header_hash = call_data
            .commit_entry(base.clone().try_into().unwrap(), POST_ID)
            .await;

        let target_header_hash = call_data
            .commit_entry(target.clone().try_into().unwrap(), POST_ID)
            .await;

        fake_authority(target_header_hash.clone().into(), &call_data).await;
        fake_authority(base_header_hash.clone().into(), &call_data).await;

        // Link the entries
        let link_add_hash = call_data
            .create_link(
                base_entry_hash.clone(),
                target_entry_hash.clone(),
                link_tag.clone(),
            )
            .await;

        fake_authority(link_add_hash.clone().into(), &call_data).await;

        link_add_hash
    };

    // Alice get links from bob
    let links = {
        let call_data = HostFnCaller::create(&alice_cell_id, &handle, &dna_file).await;

        call_data
            .get_links(base_entry_hash.clone(), None, link_options.clone())
            .await
    };

    assert_eq!(links.len(), 1);

    let expt = Link {
        target: target_entry_hash.clone(),
        timestamp: links.get(0).unwrap().timestamp.clone(),
        tag: link_tag.clone(),
        create_link_hash: link_add_hash.clone(),
    };
    assert_eq!(*links.get(0).unwrap(), expt);

    // Remove the link
    {
        let call_data = HostFnCaller::create(&bob_cell_id, &handle, &dna_file).await;

        // Link the entries
        let link_remove_hash = call_data.delete_link(link_add_hash.clone()).await;

        fake_authority(link_remove_hash.clone().into(), &call_data).await;
    }

    let links = {
        let call_data = HostFnCaller::create(&alice_cell_id, &handle, &dna_file).await;

        call_data
            .get_link_details(
                base_entry_hash.clone(),
                link_tag.clone(),
                link_options.clone(),
            )
            .await
    };

    assert_eq!(links.len(), 1);
    let (link_add, link_removes) = links.get(0).unwrap().clone();
    assert_eq!(link_removes.len(), 1);
    let link_remove = link_removes.get(0).unwrap().clone();
    let link_remove = unwrap_to::unwrap_to!(link_remove.header() => Header::DeleteLink).clone();
    let link_add = unwrap_to::unwrap_to!(link_add.header() => Header::CreateLink).clone();
    assert_eq!(link_add.tag, link_tag);
    assert_eq!(link_add.target_address, target_entry_hash);
    assert_eq!(link_add.base_address, base_entry_hash);
    assert_eq!(
        link_remove.link_add_address,
        HeaderHash::with_data_sync(&Header::CreateLink(link_add))
    );

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

struct Shutdown {
    handle: JoinHandle<()>,
    kill: oneshot::Sender<()>,
    network: HolochainP2pRef,
}

impl Shutdown {
    async fn clean(self) {
        let Self {
            handle,
            kill,
            network,
        } = self;
        kill.send(()).ok();
        // Give the network some time to clean up but don't block tests if it doesn't
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            network.ghost_actor_shutdown(),
        )
        .await
        .ok();
        tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .ok();
    }
}

/*
/// Run a test network handler which accepts two data sources to draw from.
/// It only handles Get and GetMeta requests.
/// - When handling a Get, it pulls the corresponding Element from the `element_fixt_store`
/// - When handling a GetMeta, it pulls the corresponding `TimedHeaderHash` from the `meta_fixt_store
///    and constructs a `MetadataSet` containing only that single `TimedHeaderHash`
async fn run_fixt_network(
    element_fixt_store: BTreeMap<HeaderHash, Element>,
    meta_fixt_store: BTreeMap<AnyDhtHash, TimedHeaderHash>,
) -> (HolochainP2pCell, Shutdown) {
    // Create the network
    let (network, mut recv, cell_network) = test_network(None, None).await;
    let (kill, killed) = tokio::sync::oneshot::channel();

    // Return fixt store data to gets
    let handle = tokio::task::spawn({
        async move {
            use tokio::stream::StreamExt;
            let mut killed = killed.into_stream();
            while let Either::Right((Some(evt), _)) =
                futures::future::select(killed.next(), recv.next()).await
            {
                use holochain_p2p::event::HolochainP2pEvent::*;
                debug!(?evt);
                match evt {
                    Get {
                        dht_hash, respond, ..
                    } => {
                        let dht_hash = match dht_hash.hash_type() {
                            AnyDht::Header => dht_hash.into(),
                            _ => unreachable!(),
                        };

                        let chain_element = element_fixt_store
                            .get(&dht_hash)
                            .cloned()
                            .map(|element| {
                                GetElementResponse::GetHeader(Some(Box::new(
                                    WireElement::from_element(
                                        ElementStatus::new(element, ValidationStatus::Valid),
                                        vec![],
                                        vec![],
                                    ),
                                )))
                                .try_into()
                                .unwrap()
                            })
                            .unwrap();
                        respond.respond(Ok(async move { Ok(chain_element) }.boxed().into()));
                    }
                    GetMeta {
                        dht_hash,
                        // TODO; Use options
                        options: _options,
                        respond,
                        ..
                    } => {
                        let header_hash = meta_fixt_store.get(&dht_hash).cloned().unwrap();
                        let metadata = MetadataSet {
                            headers: btreeset! {header_hash},
                            deletes: btreeset! {},
                            updates: btreeset! {},
                            invalid_headers: btreeset! {},
                            entry_dht_status: None,
                        };
                        respond.respond(Ok(async move { Ok(metadata.try_into().unwrap()) }
                            .boxed()
                            .into()));
                    }
                    _ => (),
                }
            }
        }
    });
    (
        cell_network,
        Shutdown {
            handle,
            kill,
            network,
        },
    )
}
*/

async fn generate_fixt_store() -> (
    BTreeMap<HeaderHash, Element>,
    BTreeMap<AnyDhtHash, TimedHeaderHash>,
) {
    let mut store = BTreeMap::new();
    let mut meta_store = BTreeMap::new();
    let entry = EntryFixturator::new(AppEntry).next().unwrap();
    let entry_hash = EntryHashed::from_content_sync(entry.clone()).into_hash();
    let mut element_create = fixt!(Create);
    let entry_type = AppEntryTypeFixturator::new(EntryVisibility::Public)
        .map(EntryType::App)
        .next()
        .unwrap();
    element_create.entry_type = entry_type;
    element_create.entry_hash = entry_hash.clone();
    let header = HeaderHashed::from_content_sync(Header::Create(element_create));
    let hash = header.as_hash().clone();
    let signed_header = SignedHeaderHashed::with_presigned(header, fixt!(Signature));
    meta_store.insert(
        entry_hash.into(),
        TimedHeaderHash {
            timestamp: timestamp::now(),
            header_hash: hash.clone(),
        },
    );
    store.insert(hash, Element::new(signed_header, Some(entry)));
    (store, meta_store)
}

async fn fake_authority(hash: AnyDhtHash, call_data: &HostFnCaller) {
    // Check bob can get the entry
    let element = call_data
        .get(hash.clone().into(), GetOptions::content())
        .await
        .unwrap();

    let mut element_vault = ElementBuf::vault(call_data.env.clone().into(), false).unwrap();
    let mut meta_vault = MetadataBuf::vault(call_data.env.clone().into()).unwrap();

    // Write to the meta vault to fake being an authority
    let (shh, e) = element.clone().into_inner();
    element_vault.put(shh, option_entry_hashed(e)).unwrap();

    // TODO: figure this out
    integrate_to_integrated(&element, &element_vault, &mut meta_vault)
        .await
        .unwrap();

    call_data
        .env
        .guard()
        .with_commit(|writer| {
            element_vault.flush_to_txn(writer)?;
            meta_vault.flush_to_txn(writer)
        })
        .unwrap();
}

async fn integrate_to_integrated<C: MetadataBufT<IntegratedPrefix>>(
    element: &Element,
    element_store: &ElementBuf<IntegratedPrefix>,
    meta_store: &mut C,
) -> DhtOpConvertResult<()> {
    // Produce the light directly
    for op in produce_op_lights_from_elements(vec![element])? {
        // we don't integrate element data, because it is already in our vault.
        integrate_single_metadata(op, element_store, meta_store)?
    }
    Ok(())
}
