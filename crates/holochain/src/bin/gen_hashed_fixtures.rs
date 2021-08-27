use holo_hash::generate_fixtures::*;
use holo_hash::*;
use holochain_types::prelude::*;

fn main() {
    // use rand::Rng;
    // let mut rng = rand::thread_rng();
    // let NOISE: Vec<u8> = std::iter::repeat_with(|| rng.gen())
    //     .take(10_000_000)
    //     .collect();
    let mut u = arbitrary::Unstructured::new(&NOISE);

    let ops: HashedFixtures<DhtOp> = gen_hashed_fixtures(256, &mut u, |op: &HoloHashed<DhtOp>| {
        op.as_content().dht_basis().get_loc()
    });

    let agents: HashedFixtures<AgentPubKey> =
        gen_hashed_fixtures(256, &mut u, |agent: &HoloHashed<AgentPubKey>| {
            agent.as_hash().get_loc()
        });

    dbg!(ops
        .items
        .into_iter()
        .map(|op| op.dht_basis().get_loc())
        .collect::<Vec<_>>());
    dbg!(agents
        .items
        .into_iter()
        .map(|agent| agent.as_hash().get_loc())
        .collect::<Vec<_>>());
}
