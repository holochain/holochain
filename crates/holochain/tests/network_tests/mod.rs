#![cfg(feature = "test_utils")]
#![cfg(todo_redo_old_tests)]
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
use holo_hash::ActionHash;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holochain::conductor::interface::websocket::test_utils::setup_app;
use holochain::core::workflow::produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertResult;
use holochain::core::workflow::CallZomeWorkspace;
use holochain::test_utils::test_network;
use holochain_cascade::integrate_single_metadata;
use holochain_p2p::actor::GetLinksOptions;
use holochain_p2p::actor::GetMetaOptions;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pRef;
use holochain_serialized_bytes::SerializedBytes;
use holochain_sqlite::db::ReadManager;
use holochain_sqlite::prelude::BufferedStore;
use holochain_sqlite::prelude::IntegratedPrefix;
use holochain_sqlite::prelude::WriteManager;
use holochain_state::metadata::MetadataBuf;
use holochain_state::metadata::MetadataBufT;
use holochain_state::commit_buf::CommitBuf;
use holochain_types::prelude::*;
use holochain_types::prelude::*;

use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::ActionHashed;
use holochain_zome_types::Entry;
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

// These tests look like they should be in the cascade?

/*
#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky"]
async fn get_updates_cache() {
    observability::test_run().ok();
    // Database setup
    let test_db = test_cell_db();
    let db = test_db.db();

    let (commit_fixt_store, _) = generate_fixt_store().await;
    let expected = commit_fixt_store
        .iter()
        .next()
        .map(|(h, e)| (h.clone(), e.clone()))
        .unwrap();

    // Create the cascade
    let mut workspace = CallZomeWorkspace::new(db.clone().into()).unwrap();
    let (network, shutdown) = run_fixt_network(commit_fixt_store, BTreeMap::new()).await;

    {
        // Construct the cascade with a network
        let mut cascade = workspace.cascade(network);

        // Call fetch commit
        cascade
            .fetch_commit_via_action(expected.0.clone().into(), Default::default())
            .await
            .unwrap();
    }

    // Check the cache has been updated
    let result = workspace
        .commit_cache
        .get_commit(&expected.0)
        .unwrap()
        .unwrap();
    assert_eq!(result.action(), expected.1.action());
    assert_eq!(result.entry(), expected.1.entry());

    shutdown.clean().await;
}
*/

/*
#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky!"]
async fn get_meta_updates_meta_cache() {
    observability::test_run().ok();
    // Database setup
    let test_db = test_cell_db();
    let db = test_db.db();

    // Setup other metadata store with fixtures attached
    // to known entry hash
    let (_, meta_fixt_store) = generate_fixt_store().await;
    let expected = meta_fixt_store
        .iter()
        .next()
        .map(|(h, e)| (h.clone(), e.clone()))
        .unwrap();

    // Create the cascade
    let mut workspace = CallZomeWorkspace::new(db.clone().into()).unwrap();
    let (network, shutdown) = run_fixt_network(BTreeMap::new(), meta_fixt_store).await;

    let returned = {
        // Construct the cascade with a network
        let mut cascade = workspace.cascade(network);

        // Create GetMetaOptions
        let options = GetMetaOptions::default();

        // Call fetch commit
        cascade
            .fetch_meta(expected.0.clone().into(), options)
            .await
            .unwrap()
            .first()
            .cloned()
            .unwrap()
    };

    // Check the returned commit is correct
    assert_eq!(returned.actions.len(), 1);
    assert_eq!(returned.actions.into_iter().next().unwrap(), expected.1);
    let result = {
        let mut g = db.conn();
let mut reader = g.reader().unwrap();

        // Check the cache has been updated
        workspace
            .meta_cache
            .get_actions(
                &mut reader,
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

#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky"]
async fn get_from_another_agent() {
    observability::test_run().ok();
    let dna_file = DnaFile::new(
        DnaDef {
            name: "dht_get_test".to_string(),
            uid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
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

    let mut ribosome_store = MockRibosomeStore::single_dna(dna_file, 2, 2);
    ribosome_store.expect_get_entry_def().return_const(None);

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        ribosome_store,
    )
    .await;

    let options = GetOptions::latest();

    // Bob store commit
    let entry = Post("Bananas are good for you".into());
    let entry_hash = Entry::try_from(entry.clone()).unwrap().to_hash();
    let action_hash = {
        let call_data = HostFnCaller::create(&bob_cell_id, &handle, &dna_file).await;
        let action_hash = call_data
            .commit_entry(entry.clone().try_into().unwrap(), POST_ID)
            .await;

        // Bob is not an authority yet
        // Make Bob an "authority"
        fake_authority(action_hash.clone().into(), &call_data).await;
        action_hash
    };

    // Alice get commit from bob
    let commit = {
        let call_data = HostFnCaller::create(&alice_cell_id, &handle, &dna_file).await;
        call_data
            .get(entry_hash.clone().into(), options.clone())
            .await
    };

    let (signed_action, ret_entry) = commit.unwrap().into_inner();

    // TODO: Check signed action is the same action

    // Check Bob is the author
    assert_eq!(*signed_action.action().author(), bob_agent_id);

    // Check entry is the same
    let ret_entry: Post = ret_entry.into_option().unwrap().try_into().unwrap();
    assert_eq!(entry, ret_entry);

    let new_entry = Post("Bananas are bendy".into());
    let (remove_hash, update_hash) = {
        let call_data = HostFnCaller::create(&bob_cell_id, &handle, &dna_file).await;
        let remove_hash = call_data.delete_entry(action_hash.clone()).await;

        fake_authority(remove_hash.clone().into(), &call_data).await;
        let update_hash = call_data
            .update_entry(
                new_entry.clone().try_into().unwrap(),
                POST_ID,
                action_hash.clone(),
            )
            .await;
        fake_authority(update_hash.clone().into(), &call_data).await;
        (remove_hash, update_hash)
    };

    // Alice get commit from bob
    let (entry_details, action_details) = {
        let call_data = HostFnCaller::create(&alice_cell_id, &handle, &dna_file).await;
        debug!(the_entry_hash = ?entry_hash);
        let entry_details = call_data
            .get_details(entry_hash.into(), options.clone())
            .await
            .unwrap();
        let action_details = call_data
            .get_details(action_hash.clone().into(), options.clone())
            .await
            .unwrap();
        (entry_details, action_details)
    };

    let entry_details = unwrap_to!(entry_details => Details::Entry).clone();
    let action_details = unwrap_to!(action_details => Details::Commit).clone();

    assert_eq!(Post::try_from(entry_details.entry).unwrap(), entry);
    assert_eq!(entry_details.actions.len(), 1);
    assert_eq!(entry_details.deletes.len(), 1);
    assert_eq!(entry_details.updates.len(), 1);
    assert_eq!(entry_details.entry_dht_status, EntryDhtStatus::Dead);
    assert_eq!(
        *entry_details.actions.get(0).unwrap().action_address(),
        action_hash
    );
    assert_eq!(
        *entry_details.deletes.get(0).unwrap().action_address(),
        remove_hash
    );
    assert_eq!(
        *entry_details.updates.get(0).unwrap().action_address(),
        update_hash
    );

    assert_eq!(action_details.deletes.len(), 1);
    assert_eq!(*action_details.commit.action_address(), action_hash);
    assert_eq!(
        *entry_details.deletes.get(0).unwrap().action_address(),
        remove_hash
    );

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky for some reason"]
async fn get_links_from_another_agent() {
    observability::test_run().ok();
    let dna_file = DnaFile::new(
        DnaDef {
            name: "dht_get_test".to_string(),
            uid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
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

    let mut ribosome_store = MockRibosomeStore::single_dna(dna_file, 2, 2);
    ribosome_store.expect_get_entry_def().return_const(None);

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        ribosome_store,
    )
    .await;

    let link_options = GetLinksOptions { timeout_ms: None };

    // Bob store links
    let base = Post("Bananas are good for you".into());
    let target = Post("Potassium is radioactive".into());
    let base_entry_hash = Entry::try_from(base.clone()).unwrap().to_hash();
    let target_entry_hash = Entry::try_from(target.clone()).unwrap().to_hash();
    let link_tag = fixt!(LinkTag);
    let link_add_hash = {
        let call_data = HostFnCaller::create(&bob_cell_id, &handle, &dna_file).await;
        let base_action_hash = call_data
            .commit_entry(base.clone().try_into().unwrap(), POST_ID)
            .await;

        let target_action_hash = call_data
            .commit_entry(target.clone().try_into().unwrap(), POST_ID)
            .await;

        fake_authority(target_action_hash.clone().into(), &call_data).await;
        fake_authority(base_action_hash.clone().into(), &call_data).await;

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
    let link_remove = unwrap_to::unwrap_to!(link_remove.action() => Action::DeleteLink).clone();
    let link_add = unwrap_to::unwrap_to!(link_add.action() => Action::CreateLink).clone();
    assert_eq!(link_add.tag, link_tag);
    assert_eq!(link_add.target_address, target_entry_hash);
    assert_eq!(link_add.base_address, base_entry_hash);
    assert_eq!(
        link_remove.link_add_address,
        ActionHash::with_data_sync(&Action::CreateLink(link_add))
    );

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap().unwrap();
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
/// - When handling a Get, it pulls the corresponding Commit from the `commit_fixt_store`
/// - When handling a GetMeta, it pulls the corresponding `TimedActionHash` from the `meta_fixt_store
///    and constructs a `MetadataSet` containing only that single `TimedActionHash`
async fn run_fixt_network(
    commit_fixt_store: BTreeMap<ActionHash, Commit>,
    meta_fixt_store: BTreeMap<AnyDhtHash, TimedActionHash>,
) -> (HolochainP2pDna, Shutdown) {
    // Create the network
    let (network, mut recv, dna_network) = test_network(None, None).await;
    let (kill, killed) = tokio::sync::oneshot::channel();

    // Return fixt store data to gets
    let handle = tokio::task::spawn({
        async move {
            use tokio_stream::StreamExt;
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
                            AnyDht::Action => dht_hash.into(),
                            _ => unreachable!(),
                        };

                        let chain_commit = commit_fixt_store
                            .get(&dht_hash)
                            .cloned()
                            .map(|commit| {
                                GetCommitResponse::GetAction(Some(Box::new(
                                    WireCommit::from_commit(
                                        CommitStatus::new(commit, ValidationStatus::Valid),
                                        vec![],
                                        vec![],
                                    ),
                                )))
                                .try_into()
                                .unwrap()
                            })
                            .unwrap();
                        respond.respond(Ok(async move { Ok(chain_commit) }.boxed().into()));
                    }
                    GetMeta {
                        dht_hash,
                        // TODO; Use options
                        options: _options,
                        respond,
                        ..
                    } => {
                        let action_hash = meta_fixt_store.get(&dht_hash).cloned().unwrap();
                        let metadata = MetadataSet {
                            actions: btreeset! {action_hash},
                            deletes: btreeset! {},
                            updates: btreeset! {},
                            invalid_actions: btreeset! {},
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
        dna_network,
        Shutdown {
            handle,
            kill,
            network,
        },
    )
}
*/

async fn generate_fixt_store() -> (
    BTreeMap<ActionHash, Commit>,
    BTreeMap<AnyDhtHash, TimedActionHash>,
) {
    let mut store = BTreeMap::new();
    let mut meta_store = BTreeMap::new();
    let entry = EntryFixturator::new(AppEntry).next().unwrap();
    let entry_hash = EntryHashed::from_content_sync(entry.clone()).into_hash();
    let mut commit_create = fixt!(Create);
    let entry_type = AppEntryTypeFixturator::new(EntryVisibility::Public)
        .map(EntryType::App)
        .next()
        .unwrap();
    commit_create.entry_type = entry_type;
    commit_create.entry_hash = entry_hash.clone();
    let action = ActionHashed::from_content_sync(Action::Create(commit_create));
    let hash = action.as_hash().clone();
    let signed_action = SignedActionHashed::with_presigned(action, fixt!(Signature));
    meta_store.insert(
        entry_hash.into(),
        TimedActionHash {
            timestamp: Timestamp::now(),
            action_hash: hash.clone(),
        },
    );
    store.insert(hash, Commit::new(signed_action, Some(entry)));
    (store, meta_store)
}

async fn fake_authority(hash: AnyDhtHash, call_data: &HostFnCaller) {
    // Check bob can get the entry
    let commit = call_data
        .get(hash.clone().into(), GetOptions::content())
        .await
        .unwrap();

    let mut commit_vault = CommitBuf::vault(call_data.db.clone().into(), false).unwrap();
    let mut meta_vault = MetadataBuf::vault(call_data.db.clone().into()).unwrap();

    // Write to the meta vault to fake being an authority
    let (shh, e) = commit.clone().into_inner();
    commit_vault.put(shh, option_entry_hashed(e)).unwrap();

    // TODO: figure this out
    integrate_to_integrated(&commit, &commit_vault, &mut meta_vault)
        .await
        .unwrap();

    call_data
        .db
        .conn()
        .unwrap()
        .with_commit(|writer| {
            commit_vault.flush_to_txn(writer)?;
            meta_vault.flush_to_txn(writer)
        })
        .unwrap();
}

async fn integrate_to_integrated<C: MetadataBufT<IntegratedPrefix>>(
    commit: &Commit,
    commit_store: &CommitBuf<IntegratedPrefix>,
    meta_store: &mut C,
) -> DhtOpConvertResult<()> {
    // Produce the light directly
    for op in produce_op_lights_from_commits(vec![commit])? {
        // we don't integrate commit data, because it is already in our vault.
        integrate_single_metadata(op, commit_store, meta_store)?
    }
    Ok(())
}
