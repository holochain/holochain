use super::*;
use crate::test_utils::test_network;
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use futures::future::{Either, FutureExt};
use ghost_actor::GhostControlSender;
use hdk3::prelude::EntryVisibility;
use holo_hash::*;
use holo_hash_core::hash_type::{self, AnyDht};
use holochain_p2p::{actor::GetMetaOptions, HolochainP2pCell, HolochainP2pRef};
use holochain_state::{env::ReadManager, test_utils::test_cell_env};
use holochain_types::{
    element::{ChainElement, WireElement},
    fixt::*,
    header::EntryType,
    metadata::TimeHeaderHash,
    observability, Header, HeaderHashed, Timestamp,
};
use maplit::btreeset;
use std::collections::BTreeMap;
use std::convert::TryInto;
use tokio::{sync::oneshot, task::JoinHandle};

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
    let (element_vault, meta_vault, mut element_cache, mut meta_cache) =
        test_dbs_and_mocks(&reader, &dbs);
    let (network, shutdown) = run_fixt_network(element_fixt_store, BTreeMap::new()).await;

    let returned_element = {
        // Construct the cascade with a network
        let mut cascade = Cascade::new(
            &element_vault,
            &meta_vault,
            &mut element_cache,
            &mut meta_cache,
            network,
        );

        // Call fetch element
        cascade
            .fetch_element(expected.0.clone().into(), Default::default())
            .await
            .unwrap()
            .unwrap()
    };

    // Check the returned element is correct
    assert_eq!(returned_element.header(), expected.1.header());
    assert_eq!(returned_element.entry(), expected.1.entry());

    // Check the cache has been updated
    let result = element_cache
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
    let mut meta_cache = MetadataBuf::cache(&reader, &dbs).unwrap();
    let (element_vault, meta_vault, mut element_cache, _) = test_dbs_and_mocks(&reader, &dbs);
    let (network, shutdown) = run_fixt_network(BTreeMap::new(), meta_fixt_store).await;

    let returned = {
        // Construct the cascade with a network
        let mut cascade = Cascade::new(
            &element_vault,
            &meta_vault,
            &mut element_cache,
            &mut meta_cache,
            network,
        );

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
    let result = meta_cache
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
/// - When handling a Get, it pulls the corresponding ChainElement from the `element_fixt_store`
/// - When handling a GetMeta, it pulls the corresponding `TimeHeaderHash` from the `meta_fixt_store
///    and constructs a `MetadataSet` containing only that single `TimeHeaderHash`
async fn run_fixt_network(
    element_fixt_store: BTreeMap<HeaderHash, ChainElement>,
    meta_fixt_store: BTreeMap<AnyDhtHash, TimeHeaderHash>,
) -> (HolochainP2pCell, Shutdown) {
    // Create the network
    let (network, mut recv, cell_network) = test_network().await;
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
                            .map(|element| WireElement::from_element(element).try_into().unwrap())
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
                    _ => panic!("unexpected event"),
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
    BTreeMap<HeaderHash, ChainElement>,
    BTreeMap<AnyDhtHash, TimeHeaderHash>,
) {
    let mut store = BTreeMap::new();
    let mut meta_store = BTreeMap::new();
    let entry = fixt!(Entry);
    let entry_hash = EntryHashed::with_data(entry.clone())
        .await
        .unwrap()
        .into_hash();
    let mut element_create = fixt!(EntryCreate);
    let entry_type = AppEntryTypeFixturator::new(EntryVisibility::Public)
        .map(EntryType::App)
        .next()
        .unwrap();
    element_create.entry_type = entry_type;
    element_create.entry_hash = entry_hash.clone();
    let header = HeaderHashed::with_data(Header::EntryCreate(element_create))
        .await
        .unwrap();
    let hash = header.as_hash().clone();
    let signed_header = SignedHeaderHashed::with_presigned(header, fixt!(Signature));
    meta_store.insert(
        entry_hash.into(),
        TimeHeaderHash {
            timestamp: Timestamp::now(),
            header_hash: hash.clone(),
        },
    );
    store.insert(hash, ChainElement::new(signed_header, Some(entry)));
    (store, meta_store)
}
