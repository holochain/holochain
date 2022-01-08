pub type Hash32 = [u8; 32];

pub fn fake_hash() -> Hash32 {
    use rand::distributions::*;

    let mut rng = rand::thread_rng();
    let uni = Uniform::from(u8::MIN..=u8::MAX);
    let bytes: Vec<u8> = uni.sample_iter(&mut rng).take(32).collect();
    let bytes: [u8; 32] = bytes.try_into().unwrap();
    bytes
}

#[derive(
    Copy,
    Clone,
    Debug,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    derive_more::Constructor,
    derive_more::From,
)]
pub struct OpHash(pub Hash32);

#[derive(
    Copy,
    Clone,
    Debug,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    derive_more::Constructor,
    derive_more::From,
)]
pub struct AgentKey(pub Hash32);

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    derive_more::Constructor,
    derive_more::Deref,
    derive_more::DerefMut,
)]
pub struct RegionHash(Hash32);
