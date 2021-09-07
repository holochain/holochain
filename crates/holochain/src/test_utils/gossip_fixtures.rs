//! Defines fixture data used for gossip tests.

use holo_hash::hashed_fixtures::*;
use holo_hash::*;
use holochain_types::prelude::*;

const FIXTURE_PATH: &'static str = "fixtures/gossip-fixtures.msgpack";

/// Fixture data to be used in gossip tests.
/// The ops and agents are generated such that their DHT locations are
/// evenly distributed around the u32 location space.
/// See `holo_hash::hashed_fixtures` for more info.
#[derive(Debug, Serialize, Deserialize)]
pub struct GossipFixtures {
    /// DhtOp fixtures
    pub ops: HashedFixtures<DhtOp>,
    /// AgentPubKey fixtures
    pub agents: HashedFixtures<AgentPubKey>,
}

/// Lazy static GossipFixtures, cached on disk
pub static GOSSIP_FIXTURES: once_cell::sync::Lazy<GossipFixtures> =
    once_cell::sync::Lazy::new(|| match std::fs::read(&FIXTURE_PATH) {
        Ok(bytes) => holochain_serialized_bytes::decode(&bytes).unwrap(),
        Err(_) => {
            let fixtures = gen();
            let bytes = holochain_serialized_bytes::encode(&fixtures).unwrap();
            std::fs::write(&FIXTURE_PATH, &bytes).unwrap();
            fixtures
        }
    });

fn gen() -> GossipFixtures {
    // hopefully 10MB of entropy is enough
    let noise = generate_noise(10_000_000);
    let mut u = arbitrary::Unstructured::new(&noise);

    let ops: HashedFixtures<DhtOp> =
        HashedFixtures::generate(256, &mut u, |op: &HoloHashed<DhtOp>| {
            op.as_content().dht_basis().get_loc()
        });

    let agents: HashedFixtures<AgentPubKey> =
        HashedFixtures::generate(256, &mut u, |agent: &HoloHashed<AgentPubKey>| {
            agent.as_hash().get_loc()
        });

    GossipFixtures { agents, ops }
}
