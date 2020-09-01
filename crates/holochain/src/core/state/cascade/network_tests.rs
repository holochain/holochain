use crate::{
    conductor::{dna_store::MockDnaStore, interface::websocket::test::setup_app, ConductorHandle},
    core::{
        ribosome::{host_fn, wasm_ribosome::WasmRibosome, CallContext, ZomeCallHostAccess},
        state::{
            element_buf::ElementBuf,
            metadata::{LinkMetaKey, MetadataBuf, MetadataBufT},
            workspace::Workspace,
        },
        workflow::{
            integrate_dht_ops_workflow::integrate_to_cache, CallZomeWorkspace,
            CallZomeWorkspaceLock,
        },
    },
    test_utils::test_network,
};
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use futures::future::{Either, FutureExt};
use ghost_actor::GhostControlSender;
use hdk3::prelude::{EntryError, EntryVisibility};
use holo_hash::{
    hash_type::{self, AnyDht},
    AnyDhtHash, EntryHash, HasHash, HeaderHash,
};
use holochain_keystore::KeystoreSender;
use holochain_p2p::{
    actor::{GetLinksOptions, GetMetaOptions, GetOptions, HolochainP2pRefToCell},
    HolochainP2pCell, HolochainP2pRef,
};
use holochain_serialized_bytes::prelude::*;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::{
    env::{EnvironmentWrite, ReadManager},
    prelude::{BufferedStore, GetDb, WriteManager},
    test_utils::test_cell_env,
};
use holochain_types::{
    app::InstalledCell,
    cell::CellId,
    dna::{DnaDef, DnaFile},
    element::{Element, GetElementResponse, WireElement},
    entry::option_entry_hashed,
    fixt::*,
    metadata::{MetadataSet, TimedHeaderHash},
    observability,
    test_utils::{fake_agent_pubkey_1, fake_agent_pubkey_2},
    Entry, EntryHashed, HeaderHashed, Timestamp,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{
    element::SignedHeaderHashed,
    entry_def,
    header::*,
    link::{Link, LinkTag},
    metadata::{Details, EntryDhtStatus},
    zome::ZomeName,
    CommitEntryInput, DeleteEntryInput, GetDetailsInput, GetInput, GetLinksInput, LinkEntriesInput,
    RemoveLinkInput, UpdateEntryInput,
};
use maplit::btreeset;
use std::collections::BTreeMap;
use std::{
    convert::{TryFrom, TryInto},
    sync::Arc,
};
use tokio::{sync::oneshot, task::JoinHandle};
use tracing::*;
use unwrap_to::unwrap_to;

#[tokio::test(threaded_scheduler)]
async fn get_updates_cache() {
    observability::test_run().ok();
    // Database setup
    let test_env = test_cell_env();
    let env = test_env.env();
    let dbs = env.dbs();

    let (element_fixt_store, _) = generate_fixt_store().await;
    let expected = element_fixt_store
        .iter()
        .next()
        .map(|(h, e)| (h.clone(), e.clone()))
        .unwrap();

    // Create the cascade
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), &dbs)
        .await
        .unwrap();
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
        .cache_cas
        .get_element(&expected.0)
        .unwrap()
        .unwrap();
    assert_eq!(result.header(), expected.1.header());
    assert_eq!(result.entry(), expected.1.entry());

    shutdown.clean().await;
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn get_meta_updates_meta_cache() {
    observability::test_run().ok();
    // Database setup
    let test_env = test_cell_env();
    let env = test_env.env();
    let dbs = env.dbs();
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
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), &dbs)
        .await
        .unwrap();
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
            .cache_meta
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
    assert_eq!(result[0], expected.1);
    assert_eq!(result.len(), 1);

    shutdown.clean().await;
}

#[tokio::test(threaded_scheduler)]
#[ignore]
async fn get_from_another_agent() {
    observability::test_run().ok();
    let dna_file = DnaFile::new(
        DnaDef {
            name: "dht_get_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::CommitEntry.into()].into(),
        },
        vec![TestWasm::CommitEntry.into()],
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

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        dna_store,
    )
    .await;

    let options = GetOptions {
        remote_agent_count: None,
        timeout_ms: None,
        as_race: false,
        race_timeout_ms: None,
        follow_redirects: false,
        all_live_headers_with_metadata: false,
    };

    // Bob store element
    let entry = Post("Bananas are good for you".into());
    let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
    let header_hash = {
        let (bob_env, call_data) =
            make_call_data(bob_cell_id.clone(), handle.clone(), dna_file.clone()).await;
        let dbs = bob_env.dbs();
        let header_hash = commit_entry(
            bob_env.clone(),
            &dbs,
            call_data.clone(),
            entry.clone().try_into().unwrap(),
            "post".into(),
        )
        .await;

        // Bob is not an authority yet
        // Make Bob an "authority"
        fake_authority(
            bob_env.clone(),
            &dbs,
            header_hash.clone().into(),
            call_data.clone(),
        )
        .await;
        header_hash
    };

    // Alice get element from bob
    let element = {
        let (alice_env, call_data) =
            make_call_data(alice_cell_id.clone(), handle.clone(), dna_file.clone()).await;
        let dbs = alice_env.dbs();
        get(
            alice_env.clone(),
            &dbs,
            call_data,
            entry_hash.clone().into(),
            options.clone(),
        )
        .await
    };

    let (signed_header, ret_entry) = element.unwrap().into_inner();

    // TODO: Check signed header is the same header

    // Check Bob is the author
    assert_eq!(*signed_header.header().author(), bob_agent_id);

    // Check entry is the same
    let ret_entry: Post = ret_entry.unwrap().try_into().unwrap();
    assert_eq!(entry, ret_entry);

    let new_entry = Post("Bananas are bendy".into());
    let (remove_hash, update_hash) = {
        let (bob_env, call_data) =
            make_call_data(bob_cell_id.clone(), handle.clone(), dna_file.clone()).await;
        let dbs = bob_env.dbs();
        let remove_hash = delete_entry(
            bob_env.clone(),
            &dbs,
            call_data.clone(),
            header_hash.clone(),
        )
        .await;

        fake_authority(
            bob_env.clone(),
            &dbs,
            remove_hash.clone().into(),
            call_data.clone(),
        )
        .await;
        let update_hash = update_entry(
            bob_env.clone(),
            &dbs,
            call_data.clone(),
            new_entry.clone().try_into().unwrap(),
            "post".into(),
            header_hash.clone(),
        )
        .await;
        fake_authority(
            bob_env.clone(),
            &dbs,
            update_hash.clone().into(),
            call_data.clone(),
        )
        .await;
        (remove_hash, update_hash)
    };

    // Alice get element from bob
    let (entry_details, header_details) = {
        let (alice_env, call_data) =
            make_call_data(alice_cell_id.clone(), handle.clone(), dna_file.clone()).await;
        let dbs = alice_env.dbs();
        debug!(the_entry_hash = ?entry_hash);
        let entry_details = get_details(
            alice_env.clone(),
            &dbs,
            call_data.clone(),
            entry_hash.into(),
            options.clone(),
        )
        .await
        .unwrap();
        let header_details = get_details(
            alice_env.clone(),
            &dbs,
            call_data.clone(),
            header_hash.clone().into(),
            options.clone(),
        )
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
        HeaderHash::with_data_sync(entry_details.headers.get(0).unwrap()),
        header_hash
    );
    assert_eq!(
        HeaderHash::with_data_sync(&Header::ElementDelete(
            entry_details.deletes.get(0).unwrap().clone()
        )),
        remove_hash
    );
    assert_eq!(
        HeaderHash::with_data_sync(&Header::EntryUpdate(
            entry_details.updates.get(0).unwrap().clone()
        )),
        update_hash
    );

    assert_eq!(header_details.deletes.len(), 1);
    assert_eq!(*header_details.element.header_address(), header_hash);
    assert_eq!(
        HeaderHash::with_data_sync(&Header::ElementDelete(
            header_details.deletes.get(0).unwrap().clone()
        )),
        remove_hash
    );

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

#[tokio::test(threaded_scheduler)]
// @todo this is flakey for some reason
#[ignore]
async fn get_links_from_another_agent() {
    observability::test_run().ok();
    let dna_file = DnaFile::new(
        DnaDef {
            name: "dht_get_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::CommitEntry.into()].into(),
        },
        vec![TestWasm::CommitEntry.into()],
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
        let (bob_env, call_data) =
            make_call_data(bob_cell_id.clone(), handle.clone(), dna_file.clone()).await;
        let dbs = bob_env.dbs();
        let base_header_hash = commit_entry(
            bob_env.clone(),
            &dbs,
            call_data.clone(),
            base.clone().try_into().unwrap(),
            "post".into(),
        )
        .await;

        let target_header_hash = commit_entry(
            bob_env.clone(),
            &dbs,
            call_data.clone(),
            target.clone().try_into().unwrap(),
            "post".into(),
        )
        .await;

        fake_authority(
            bob_env.clone(),
            &dbs,
            target_header_hash.clone().into(),
            call_data.clone(),
        )
        .await;
        fake_authority(
            bob_env.clone(),
            &dbs,
            base_header_hash.clone().into(),
            call_data.clone(),
        )
        .await;

        // Link the entries
        let link_add_hash = link_entries(
            bob_env.clone(),
            &dbs,
            call_data.clone(),
            base_entry_hash.clone(),
            target_entry_hash.clone(),
            link_tag.clone(),
        )
        .await;

        fake_authority(
            bob_env.clone(),
            &dbs,
            link_add_hash.clone().into(),
            call_data.clone(),
        )
        .await;

        link_add_hash
    };

    // Alice get links from bob
    let links = {
        let (alice_env, call_data) =
            make_call_data(alice_cell_id.clone(), handle.clone(), dna_file.clone()).await;
        let dbs = alice_env.dbs();

        get_links(
            alice_env.clone(),
            &dbs,
            call_data.clone(),
            base_entry_hash.clone(),
            None,
            link_options.clone(),
        )
        .await
    };

    assert_eq!(links.len(), 1);

    let expt = Link {
        target: target_entry_hash.clone(),
        timestamp: links.get(0).unwrap().timestamp.clone(),
        tag: link_tag.clone(),
    };
    assert_eq!(*links.get(0).unwrap(), expt);

    // Remove the link
    {
        let (bob_env, call_data) =
            make_call_data(bob_cell_id.clone(), handle.clone(), dna_file.clone()).await;
        let dbs = bob_env.dbs();

        // Link the entries
        let link_remove_hash = remove_link(
            bob_env.clone(),
            &dbs,
            call_data.clone(),
            link_add_hash.clone(),
        )
        .await;

        fake_authority(
            bob_env.clone(),
            &dbs,
            link_remove_hash.clone().into(),
            call_data.clone(),
        )
        .await;
    }

    let links = {
        let (alice_env, call_data) =
            make_call_data(alice_cell_id.clone(), handle.clone(), dna_file.clone()).await;
        let dbs = alice_env.dbs();

        get_link_details(
            alice_env.clone(),
            &dbs,
            call_data.clone(),
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
    assert_eq!(link_add.tag, link_tag);
    assert_eq!(link_add.target_address, target_entry_hash);
    assert_eq!(link_add.base_address, base_entry_hash);
    assert_eq!(
        link_remove.link_add_address,
        HeaderHash::with_data_sync(&Header::LinkAdd(link_add))
    );

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

#[derive(Default, SerializedBytes, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
#[repr(transparent)]
#[serde(transparent)]
struct Post(String);

impl TryFrom<Post> for Entry {
    type Error = EntryError;
    fn try_from(post: Post) -> Result<Self, Self::Error> {
        Entry::app(post.try_into()?)
    }
}

impl TryFrom<Entry> for Post {
    type Error = EntryError;
    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
        let entry = unwrap_to!(entry => Entry::App).clone();
        Ok(Post::try_from(entry.into_sb())?)
    }
}

#[derive(Clone)]
struct CallData {
    ribosome: WasmRibosome,
    zome_name: ZomeName,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
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
                                    WireElement::from_element(element, None),
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

async fn generate_fixt_store() -> (
    BTreeMap<HeaderHash, Element>,
    BTreeMap<AnyDhtHash, TimedHeaderHash>,
) {
    let mut store = BTreeMap::new();
    let mut meta_store = BTreeMap::new();
    let entry = EntryFixturator::new(AppEntry).next().unwrap();
    let entry_hash = EntryHashed::from_content_sync(entry.clone()).into_hash();
    let mut element_create = fixt!(EntryCreate);
    let entry_type = AppEntryTypeFixturator::new(EntryVisibility::Public)
        .map(EntryType::App)
        .next()
        .unwrap();
    element_create.entry_type = entry_type;
    element_create.entry_hash = entry_hash.clone();
    let header = HeaderHashed::from_content_sync(Header::EntryCreate(element_create));
    let hash = header.as_hash().clone();
    let signed_header = SignedHeaderHashed::with_presigned(header, fixt!(Signature));
    meta_store.insert(
        entry_hash.into(),
        TimedHeaderHash {
            timestamp: Timestamp::now(),
            header_hash: hash.clone(),
        },
    );
    store.insert(hash, Element::new(signed_header, Some(entry)));
    (store, meta_store)
}

async fn commit_entry(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    entry: Entry,
    entry_def_id: entry_def::EntryDefId,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs)
        .await
        .unwrap();

    let input = CommitEntryInput::new((entry_def_id.clone(), entry.clone()));

    let output = {
        let (_g, workspace_lock) = CallZomeWorkspaceLock::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(workspace_lock, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::commit_entry::commit_entry(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

async fn delete_entry<'env>(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    hash: HeaderHash,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs)
        .await
        .unwrap();

    let input = DeleteEntryInput::new(hash);

    let output = {
        let (_g, workspace_lock) = CallZomeWorkspaceLock::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(workspace_lock, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        let r = host_fn::delete_entry::delete_entry(ribosome.clone(), call_context.clone(), input);
        let r = r.map_err(|e| {
            debug!(%e);
            e
        });
        r.unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

async fn update_entry<'env>(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    entry: Entry,
    entry_def_id: entry_def::EntryDefId,
    original_header_hash: HeaderHash,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs)
        .await
        .unwrap();

    let input = UpdateEntryInput::new((entry_def_id.clone(), entry.clone(), original_header_hash));

    let output = {
        let (_g, workspace_lock) = CallZomeWorkspaceLock::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(workspace_lock, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::update_entry::update_entry(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

async fn get<'env>(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    entry_hash: AnyDhtHash,
    _options: GetOptions,
) -> Option<Element> {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs)
        .await
        .unwrap();

    // let mut cascade = workspace.cascade(call_data.network);
    // cascade.dht_get(entry_hash, options).await.unwrap()

    // TODO: use the real get entry when element in zome types pr lands
    let input = GetInput::new((
        entry_hash.clone().into(),
        holochain_zome_types::entry::GetOptions,
    ));

    let output = {
        let (_g, workspace_lock) = CallZomeWorkspaceLock::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(workspace_lock, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::get::get(ribosome.clone(), call_context.clone(), input).unwrap()
    };
    output.into_inner()
}

async fn get_details<'env>(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    entry_hash: AnyDhtHash,
    _options: GetOptions,
) -> Option<Details> {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs)
        .await
        .unwrap();

    let input = GetDetailsInput::new((
        entry_hash.clone().into(),
        holochain_zome_types::entry::GetOptions,
    ));

    let output = {
        let (_g, workspace_lock) = CallZomeWorkspaceLock::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(workspace_lock, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::get_details::get_details(ribosome.clone(), call_context.clone(), input).unwrap()
    };
    output.into_inner()
}

async fn link_entries<'env>(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    base: EntryHash,
    target: EntryHash,
    link_tag: LinkTag,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs)
        .await
        .unwrap();

    let input = LinkEntriesInput::new((base.clone(), target.clone(), link_tag));

    let output = {
        let (_g, workspace_lock) = CallZomeWorkspaceLock::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(workspace_lock, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::link_entries::link_entries(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

async fn remove_link<'env>(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    link_add_hash: HeaderHash,
) -> HeaderHash {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs)
        .await
        .unwrap();

    let input = RemoveLinkInput::new(link_add_hash);

    let output = {
        let (_g, workspace_lock) = CallZomeWorkspaceLock::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(workspace_lock, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::remove_link::remove_link(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

async fn get_links<'env>(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    base: EntryHash,
    link_tag: Option<LinkTag>,
    _options: GetLinksOptions,
) -> Vec<Link> {
    let CallData {
        network,
        keystore,
        ribosome,
        zome_name,
    } = call_data;
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs)
        .await
        .unwrap();

    let input = GetLinksInput::new((base.clone(), link_tag));

    let output = {
        let (_g, workspace_lock) = CallZomeWorkspaceLock::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(workspace_lock, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::get_links::get_links(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env.guard()
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner().into()
}

async fn get_link_details<'env>(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    call_data: CallData,
    base: EntryHash,
    tag: LinkTag,
    options: GetLinksOptions,
) -> Vec<(LinkAdd, Vec<LinkRemove>)> {
    let mut workspace = CallZomeWorkspace::new(env.clone().into(), dbs)
        .await
        .unwrap();

    let mut cascade = workspace.cascade(call_data.network);
    let key = LinkMetaKey::BaseZomeTag(&base, 0.into(), &tag);
    cascade.get_link_details(&key, options).await.unwrap()
}

async fn fake_authority<'env>(
    env: EnvironmentWrite,
    dbs: &impl GetDb,
    hash: AnyDhtHash,
    call_data: CallData,
) {
    // Check bob can get the entry
    let element = get(
        env.clone(),
        dbs,
        call_data,
        hash.clone().into(),
        GetOptions::default(),
    )
    .await
    .unwrap();

    let mut element_vault = ElementBuf::vault(env.clone().into(), dbs, false).unwrap();
    let mut meta_vault = MetadataBuf::vault(env.clone().into(), dbs).unwrap();

    // Write to the meta vault to fake being an authority
    let (shh, e) = element.clone().into_inner();
    element_vault
        .put(shh, option_entry_hashed(e).await)
        .unwrap();

    integrate_to_cache(&element, &element_vault, &mut meta_vault)
        .await
        .unwrap();

    env.guard()
        .with_commit(|writer| {
            element_vault.flush_to_txn(writer)?;
            meta_vault.flush_to_txn(writer)
        })
        .unwrap();
}

async fn make_call_data(
    cell_id: CellId,
    handle: ConductorHandle,
    dna_file: DnaFile,
) -> (EnvironmentWrite, CallData) {
    let env = handle.get_cell_env(&cell_id).await.unwrap();
    let keystore = env.keystore().clone();
    let network = handle
        .holochain_p2p()
        .to_cell(cell_id.dna_hash().clone(), cell_id.agent_pubkey().clone());

    let zome_name = dna_file.dna().zomes.get(0).unwrap().0.clone();
    let ribosome = WasmRibosome::new(dna_file);
    let call_data = CallData {
        ribosome,
        zome_name,
        network,
        keystore,
    };
    (env, call_data)
}
