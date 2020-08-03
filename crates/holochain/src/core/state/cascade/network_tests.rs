use super::*;
use crate::{
    conductor::{dna_store::MockDnaStore, interface::websocket::test::setup_app},
    core::{
        ribosome::{host_fn, wasm_ribosome::WasmRibosome, CallContext, ZomeCallHostAccess},
        state::workspace::Workspace,
        workflow::{
            integrate_dht_ops_workflow::integrate_to_cache,
            unsafe_call_zome_workspace::UnsafeCallZomeWorkspace, CallZomeWorkspace,
        },
    },
    test_utils::test_network,
};
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use futures::future::{Either, FutureExt};
use ghost_actor::GhostControlSender;
use hdk3::prelude::EntryVisibility;
use holo_hash::{
    hash_type::{self, AnyDht},
    HasHash,
};
use holochain_keystore::KeystoreSender;
use holochain_p2p::{
    actor::{GetMetaOptions, HolochainP2pRefToCell},
    HolochainP2pCell, HolochainP2pRef,
};
use holochain_serialized_bytes::prelude::*;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::{
    env::{EnvironmentWriteRef, ReadManager},
    prelude::{BufferedStore, GetDb, WriteManager},
    test_utils::test_cell_env,
};
use holochain_types::{
    app::InstalledCell,
    cell::CellId,
    dna::{DnaDef, DnaFile},
    element::{Element, GetElementResponse, WireElement},
    fixt::*,
    metadata::TimedHeaderHash,
    observability,
    test_utils::{fake_agent_pubkey_1, fake_agent_pubkey_2},
    Entry, EntryHashed, HeaderHashed, Timestamp,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{entry_def, header::*, zome::ZomeName, CommitEntryInput};
use maplit::btreeset;
use std::collections::BTreeMap;
use std::{
    convert::{TryFrom, TryInto},
    sync::Arc,
};
use tokio::{sync::oneshot, task::JoinHandle};
use unwrap_to::unwrap_to;

#[tokio::test(threaded_scheduler)]
async fn get_updates_cache() {
    observability::test_run().ok();
    // Database setup
    let env = test_cell_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;
    let reader = env_ref.reader().unwrap();

    let (element_fixt_store, _) = generate_fixt_store().await;
    let expected = element_fixt_store
        .iter()
        .next()
        .map(|(h, e)| (h.clone(), e.clone()))
        .unwrap();

    // Create the cascade
    let mut workspace = CallZomeWorkspace::new(&reader, &dbs).unwrap();
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
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.header(), expected.1.header());
    assert_eq!(result.entry(), expected.1.entry());

    shutdown.clean().await;
}

#[tokio::test(threaded_scheduler)]
async fn get_meta_updates_meta_cache() {
    observability::test_run().ok();
    // Database setup
    let env = test_cell_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;
    let reader = env_ref.reader().unwrap();

    // Setup other metadata store with fixtures attached
    // to known entry hash
    let (_, meta_fixt_store) = generate_fixt_store().await;
    let expected = meta_fixt_store
        .iter()
        .next()
        .map(|(h, e)| (h.clone(), e.clone()))
        .unwrap();

    // Create the cascade
    let mut workspace = CallZomeWorkspace::new(&reader, &dbs).unwrap();
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

    // Check the cache has been updated
    let result = workspace
        .cache_meta
        .get_headers(match expected.0.hash_type().clone() {
            hash_type::AnyDht::Entry(e) => expected.0.clone().retype(e),
            _ => unreachable!(),
        })
        .unwrap()
        .collect::<Vec<_>>()
        .unwrap();

    assert_eq!(result[0], expected.1);
    assert_eq!(result.len(), 1);

    shutdown.clean().await;
}

#[tokio::test(threaded_scheduler)]
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
    let zome_name: ZomeName = TestWasm::CommitEntry.into();

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
    };

    // Bob store element
    let entry = Post("Bananas are good for you".into());
    let entry_hash = EntryHash::with_data(&Entry::try_from(entry.clone()).unwrap()).await;
    {
        let bob_env = handle.get_cell_env(&bob_cell_id).await.unwrap();
        let keystore = bob_env.keystore().clone();
        let network = handle.holochain_p2p().to_cell(
            bob_cell_id.dna_hash().clone(),
            bob_cell_id.agent_pubkey().clone(),
        );

        let ribosome = WasmRibosome::new(dna_file.clone());
        let call_data = CallData {
            ribosome,
            zome_name: zome_name.clone(),
            network,
            keystore,
        };
        let env_ref = bob_env.guard().await;
        let dbs = bob_env.dbs().await;
        commit_entry(
            &env_ref,
            &dbs,
            call_data.clone(),
            entry.clone().try_into().unwrap(),
            "post".into(),
        )
        .await;

        // Bob is not an authority yet

        // Check bob can get the entry
        let element = get_entry(
            &env_ref,
            &dbs,
            call_data,
            entry_hash.clone(),
            options.clone(),
        )
        .await
        .unwrap();

        let (signed_header, ret_entry) = element.clone().into_inner();

        // TODO: Check signed header is the same header

        // Check Bob is the author
        assert_eq!(*signed_header.header().author(), bob_agent_id);

        // Check entry is the same
        let ret_entry: Post = ret_entry.unwrap().try_into().unwrap();
        assert_eq!(entry, ret_entry);

        // Make Bob an "authority"
        fake_authority(&env_ref, &dbs, element).await;
    }

    // Alice get element from bob
    let element = {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
        let keystore = alice_env.keystore().clone();
        let network = handle.holochain_p2p().to_cell(
            alice_cell_id.dna_hash().clone(),
            alice_cell_id.agent_pubkey().clone(),
        );

        let ribosome = WasmRibosome::new(dna_file);
        let call_data = CallData {
            ribosome,
            zome_name,
            network,
            keystore,
        };
        let env_ref = alice_env.guard().await;
        let dbs = alice_env.dbs().await;
        get_entry(&env_ref, &dbs, call_data, entry_hash, options.clone()).await
    };

    let (signed_header, ret_entry) = element.unwrap().into_inner();

    // TODO: Check signed header is the same header

    // Check Bob is the author
    assert_eq!(*signed_header.header().author(), bob_agent_id);

    // Check entry is the same
    let ret_entry: Post = ret_entry.unwrap().try_into().unwrap();
    assert_eq!(entry, ret_entry);

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

#[derive(Default, SerializedBytes, Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
#[repr(transparent)]
#[serde(transparent)]
struct Post(String);

impl TryFrom<Post> for Entry {
    type Error = SerializedBytesError;
    fn try_from(post: Post) -> Result<Self, Self::Error> {
        Ok(Entry::App(post.try_into()?))
    }
}

impl TryFrom<Entry> for Post {
    type Error = SerializedBytesError;
    fn try_from(entry: Entry) -> Result<Self, Self::Error> {
        let entry = unwrap_to!(entry => Entry::App).clone();
        Ok(Post::try_from(entry)?)
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
                            AnyDht::Header => dht_hash.retype(hash_type::Header),
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
    let entry = fixt!(Entry);
    let entry_hash = EntryHashed::from_content(entry.clone()).await.into_hash();
    let mut element_create = fixt!(EntryCreate);
    let entry_type = AppEntryTypeFixturator::new(EntryVisibility::Public)
        .map(EntryType::App)
        .next()
        .unwrap();
    element_create.entry_type = entry_type;
    element_create.entry_hash = entry_hash.clone();
    let header = HeaderHashed::from_content(Header::EntryCreate(element_create)).await;
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

async fn commit_entry<'env>(
    env_ref: &'env EnvironmentWriteRef<'env>,
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
    let reader = env_ref.reader().unwrap();
    let mut workspace = CallZomeWorkspace::new(&reader, dbs).unwrap();

    let input = CommitEntryInput::new((entry_def_id.clone(), entry.clone()));

    let output = {
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
        let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
        let call_context = CallContext::new(zome_name, host_access.into());
        let ribosome = Arc::new(ribosome);
        let call_context = Arc::new(call_context);
        host_fn::commit_entry::commit_entry(ribosome.clone(), call_context.clone(), input).unwrap()
    };

    // Write
    env_ref
        .with_commit(|writer| workspace.flush_to_txn(writer))
        .unwrap();

    output.into_inner()
}

async fn get_entry<'env>(
    env_ref: &'env EnvironmentWriteRef<'env>,
    dbs: &impl GetDb,
    call_data: CallData,
    entry_hash: EntryHash,
    options: GetOptions,
) -> Option<Element> {
    let reader = env_ref.reader().unwrap();
    let mut workspace = CallZomeWorkspace::new(&reader, dbs).unwrap();

    let mut cascade = workspace.cascade(call_data.network);
    cascade.dht_get(entry_hash.into(), options).await.unwrap()

    // TODO: use the real get entry when element in zome types pr lands
    // let input = GetInput::new((entry_hash.clone().into(), GetOptions));

    // let output = {
    //     let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace);
    //     let host_access = ZomeCallHostAccess::new(raw_workspace, keystore, network);
    //     let call_context = CallContext::new(zome_name, host_access);
    //     let ribosome = Arc::new(ribosome);
    //     let call_context = Arc::new(call_context);
    //     host_fn::get_entry::get_entry(ribosome.clone(), call_context.clone(), input).unwrap()
    // };
    // output.into_inner().try_into().unwrap()
}

async fn fake_authority<'env>(
    env_ref: &'env EnvironmentWriteRef<'env>,
    dbs: &impl GetDb,
    element: Element,
) {
    let reader = env_ref.reader().unwrap();
    let mut element_vault = ElementBuf::vault(&reader, dbs, false).unwrap();
    let mut meta_vault = MetadataBuf::vault(&reader, dbs).unwrap();

    // Write to the meta vault to fake being an authority
    let (shh, e) = element.clone().into_inner();
    element_vault
        .put(shh, option_entry_hashed(e).await)
        .unwrap();

    integrate_to_cache(&element, &element_vault, &mut meta_vault)
        .await
        .unwrap();

    env_ref
        .with_commit(|writer| {
            element_vault.flush_to_txn(writer)?;
            meta_vault.flush_to_txn(writer)
        })
        .unwrap();
}
