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
    Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, derive_more::Constructor, derive_more::From,
)]
pub struct OpHash(pub Hash32);

#[derive(
    Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, derive_more::Constructor, derive_more::From,
)]
pub struct AgentKey(pub Hash32);

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    derive_more::Constructor,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct RegionHash(pub Hash32);

impl RegionHash {
    pub fn from_vec(v: Vec<u8>) -> Option<Self> {
        v.try_into().map(Self).ok()
    }

    pub fn loc(&self) -> u32 {
        u32::from_be_bytes(self.0[28..32].try_into().unwrap())
    }
}

impl std::fmt::Debug for OpHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}(0x", "OpHash"))?;
        for byte in &self.0 {
            f.write_fmt(format_args!("{:02x}", byte))?;
        }
        f.write_fmt(format_args!(")"))?;
        Ok(())
    }
}

impl std::fmt::Debug for AgentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}(0x", "AgentKey"))?;
        for byte in &self.0 {
            f.write_fmt(format_args!("{:02x}", byte))?;
        }
        f.write_fmt(format_args!(")"))?;
        Ok(())
    }
}

impl std::fmt::Debug for RegionHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}(0x", "RegionHash"))?;
        for byte in &self.0 {
            f.write_fmt(format_args!("{:02x}", byte))?;
        }
        f.write_fmt(format_args!(")"))?;
        Ok(())
    }
}
