//! Simple hash types.
//!
//! TODO: unify with hashes from `kitsune_p2p_types::bin_types`

/// 32 bytes
pub type Hash32 = [u8; 32];

/// Get a fake hash, for testing only.
#[cfg(feature = "test_utils")]
pub fn fake_hash() -> Hash32 {
    use rand::distributions::*;

    let mut rng = rand::thread_rng();
    let uni = Uniform::from(u8::MIN..=u8::MAX);
    let bytes: Vec<u8> = uni.sample_iter(&mut rng).take(32).collect();
    let bytes: [u8; 32] = bytes.try_into().unwrap();
    bytes
}

/// The hash of an Op
#[derive(
    Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, derive_more::Constructor, derive_more::From,
)]
pub struct OpHash(pub Hash32);

impl OpHash {
    /// Random fake hash for testing
    pub fn fake() -> Self {
        Self(fake_hash())
    }
}

/// The hash of an Agent
#[derive(
    Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, derive_more::Constructor, derive_more::From,
)]
pub struct AgentKey(pub Hash32);

impl AgentKey {
    /// Random fake hash for testing
    pub fn fake() -> Self {
        Self(fake_hash())
    }
}

/// The hash of a Region, which is the XOR of all OpHashes contained in this region.
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
    /// If the Vec is 32/36/39 long, construct a RegionHash from it
    pub fn from_vec(v: Vec<u8>) -> Option<Self> {
        if v.len() == 39 {
            v[4..36].try_into().map(Self).ok()
        } else if v.len() == 36 {
            v[4..36].try_into().map(Self).ok()
        } else {
            v[..].try_into().map(Self).ok()
        }
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
