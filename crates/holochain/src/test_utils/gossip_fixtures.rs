//! Fixture data to be used in gossip tests

use std::collections::HashMap;

use holo_hash::hashed_fixtures::*;
use holo_hash::*;
use holochain_types::prelude::*;
use kitsune_p2p::test_util::scenario_def::LocBucket;

const FIXTURE_PATH: &'static str = "fixtures/gossip-fixtures.msgpack";

/// Fixture data to be used in gossip tests.
/// The ops and agents are generated such that their DHT locations are
/// evenly distributed around the u32 location space.
/// See `holo_hash::hashed_fixtures` for more info.
#[derive(Debug, Serialize, Deserialize)]
pub struct GossipFixtures {
    /// The pre-generated DhtOps
    pub ops: HashedFixtures<DhtOp>,
    /// The pre-generated AgentPubKeys
    pub agents: HashedFixtures<AgentPubKey>,
}

/// Lazily-instantiated fixtures for gossip.
/// This can take some time to generate the first time, since it is brute-force
/// search for hashes that satisfy the necessary conditions.
pub static GOSSIP_FIXTURES: once_cell::sync::Lazy<GossipFixtures> =
    once_cell::sync::Lazy::new(|| {
        //match std::fs::read(&FIXTURE_PATH) {
        // Ok(bytes) => holochain_serialized_bytes::decode(&bytes).unwrap(),
        // Err(_) => {
        let fixtures = gen();
        let bytes = holochain_serialized_bytes::encode(&fixtures).unwrap();
        std::fs::write(&FIXTURE_PATH, &bytes).unwrap();
        fixtures
        // }
    });

/// Map from DhtOpHash to the CoarseLoc for the **basis hash** (not the op hash!)
pub static GOSSIP_FIXTURE_OP_LOOKUP: once_cell::sync::Lazy<HashMap<DhtOpHash, LocBucket>> =
    once_cell::sync::Lazy::new(|| {
        GOSSIP_FIXTURES
            .ops
            .items
            .iter()
            .enumerate()
            .map(|(i, op)| (op.as_hash().clone(), i as i8))
            .collect()
    });

fn gen() -> GossipFixtures {
    // hopefully 10MB of entropy is enough
    // let noise = generate_noise(10_000_000);
    let mut u = arbitrary::Unstructured::new(&NOISE);
    use holochain_types::dht_op::facts;
    let op_fact = contrafact::facts![
        contrafact::brute("header is not dna type", |op: &DhtOp| op
            .header()
            .header_type()
            != HeaderType::Dna),
        facts::header_type_matches_entry_existence(),
        facts::header_references_entry(),
    ];

    let ops: HashedFixtures<DhtOp> =
        HashedFixtures::generate(&mut u, Some(op_fact), |op: &HoloHashed<DhtOp>| {
            op.as_content().dht_basis().get_loc()
        });

    let agents: HashedFixtures<AgentPubKey> =
        HashedFixtures::generate(&mut u, None, |agent: &HoloHashed<AgentPubKey>| {
            agent.as_hash().get_loc()
        });

    GossipFixtures { agents, ops }
}
