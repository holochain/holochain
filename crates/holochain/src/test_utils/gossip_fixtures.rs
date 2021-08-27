use holo_hash::hashed_fixtures::*;
use holo_hash::*;
use holochain_types::prelude::*;

const FIXTURE_PATH: PathBuf = "fixtures/".into();

/// Fixture data to be used in gossip tests.
/// The ops and agents are generated such that their DHT locations are
/// evenly distributed around the u32 location space.
/// See `holo_hash::hashed_fixtures` for more info.
pub struct GossipFixtures {
    pub ops: HashedFixtures<DhtOp>,
    pub agents: HashedFixtures<AgentPubKey>,
}

pub async fn gossip_fixtures() -> GossipFixtures {
    match ffs::read(&FIXTURE_PATH).await {
        Ok(bytes) => holochain_serialized_bytes::decode(&bytes).unwrap(),
        Err(_) => {
            let fixtures = gen();
            ffs::write(&FIXTURE_PATH, &bytes).await.unwrap();
            fixtures
        }
    }
}

fn gen() -> GossipFixtures {
    // hopefully 10MB of entropy is enough
    let noise = generate_noise(10_000_000);
    let mut u = arbitrary::Unstructured::new(&noise);

    let ops: HashedFixtures<DhtOp> = gen_hashed_fixtures(256, &mut u, |op: &HoloHashed<DhtOp>| {
        op.as_content().dht_basis().get_loc()
    });

    let agents: HashedFixtures<AgentPubKey> =
        gen_hashed_fixtures(256, &mut u, |agent: &HoloHashed<AgentPubKey>| {
            agent.as_hash().get_loc()
        });

    GossipFixtures { agents, ops }
}
