use super::*;
use fixt::prelude::*;
use futures::future::{Either, FutureExt};
use ghost_actor::GhostControlSender;
use holo_hash::{AgentPubKeyFixturator, DnaHashFixturator, Hashed, HeaderHash};
use holochain_p2p::{
    actor::{HolochainP2pRefToCell, HolochainP2pSender},
    spawn_holochain_p2p, HolochainP2pCell, HolochainP2pRef,
};
use holochain_serialized_bytes::prelude::*;
use holochain_state::{env::ReadManager, test_utils::test_cell_env};
use holochain_types::{
    composite_hash::AnyDhtHash,
    element::{ChainElement, SignedHeader},
    Entry,
};
use std::collections::BTreeMap;
use tokio::{sync::oneshot, task::JoinHandle};
use unwrap_to::unwrap_to;

#[tokio::test(threaded_scheduler)]
async fn get_updates_cache() {
    // Database setup
    let env = test_cell_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;
    let reader = env_ref.reader().unwrap();

    let fixt_store = generate_fixt_store();
    let expected = fixt_store
        .iter()
        .next()
        .map(|(h, e)| (h.clone(), e.clone()))
        .unwrap();

    // Create the cascade
    let (element_vault, meta_vault, element_cache, meta_cache) = test_dbs_and_mocks(&reader, &dbs);
    let (network, shutdown) = run_fixt_network(fixt_store).await;
    // Construct the cascade with a network
    let cascade = Cascade::new(
        &element_vault,
        &meta_vault,
        &mut element_cache,
        &meta_cache,
        network,
    );

    // Call fetch element
    let returned_element = cascade
        .fetch_element(expected.0.clone().into(), Default::default())
        .await
        .unwrap()
        .unwrap();

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

async fn run_fixt_network(
    fixt_store: BTreeMap<HeaderHash, ChainElement>,
) -> (HolochainP2pCell, Shutdown) {
    // Create the network
    let (network, mut recv) = spawn_holochain_p2p().await.unwrap();
    let (kill, killed) = tokio::sync::oneshot::channel();
    let dna = fixt!(DnaHash);
    let agent_key = fixt!(AgentPubKey);
    let cell_network = network.to_cell(dna.clone(), agent_key.clone());
    network.join(dna, agent_key).await.unwrap();

    // Return fixt store data to gets
    let handle = tokio::task::spawn({
        async move {
            use tokio::stream::StreamExt;
            let mut killed = killed.into_stream();
            while let Either::Right((Some(evt), _)) =
                futures::future::select(killed.next(), recv.next()).await
            {
                use holochain_p2p::event::HolochainP2pEvent::*;
                match evt {
                    Get {
                        dht_hash, respond, ..
                    } => {
                        let dht_hash = unwrap_to!(dht_hash => AnyDhtHash::Header);
                        let chain_element = fixt_store
                            .get(dht_hash)
                            .cloned()
                            .map(|element| {
                                let (header, entry) = element.into_inner();
                                let val = PlaceholderGetReturn {
                                    signed_header: header.into_content(),
                                    entry,
                                };
                                val.try_into().unwrap()
                            })
                            .unwrap();
                        respond.respond(Ok(async move { Ok(chain_element) }.boxed().into()));
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

fn generate_fixt_store() -> BTreeMap<HeaderHash, ChainElement> {
    todo!()
}
