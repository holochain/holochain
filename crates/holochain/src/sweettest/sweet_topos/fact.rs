/// Defines contrafact facts for density of a network.
pub mod density;
/// Defines contrafact facts for generating a network edge.
pub mod edge;
/// Defines contrafact facts for generating a network node.
pub mod node;
/// Defines contrafact facts for partitioning a network.
pub mod partition;
/// Defines contrafact facts for sizing a network.
pub mod size;

use contrafact::Generator;
use rand::Rng;
use rand::SeedableRng;

/// Create a random number generator from a contrafact generator.
/// Probably this should be lifted upstream to contrafact.
pub fn rng_from_generator(g: &mut Generator) -> impl Rng {
    let seed: [u8; 32] = g
        .bytes(32)
        .expect("failed to seed rng from generator")
        .try_into()
        .unwrap();
    rand_chacha::ChaCha8Rng::from_seed(seed)
}
