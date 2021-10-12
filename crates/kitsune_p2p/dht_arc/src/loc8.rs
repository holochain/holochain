use crate::DhtLocation;

const F: u32 = 16777216;

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, derive_more::From, derive_more::Display,
)]
#[display(fmt = "{}", _0)]
pub struct Loc8(i8);

impl std::fmt::Debug for Loc8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Loc8 {
    pub fn as_i8(&self) -> i8 {
        self.0
    }

    pub fn as_u8(&self) -> u8 {
        self.0 as u8
    }

    pub fn vec<L: Into<Loc8>, I: IntoIterator<Item = L>>(it: I) -> Vec<Self> {
        it.into_iter().map(Into::into).collect()
    }
}

impl From<Loc8> for DhtLocation {
    fn from(i: Loc8) -> Self {
        DhtLocation::new(i.0 as u8 as u32 * F)
    }
}

impl DhtLocation {
    pub fn as_loc8(&self) -> Loc8 {
        Loc8((self.as_u32() / F) as u8 as i8)
    }
}

impl DhtLocation {
    /// Turn this location into a "representative" 36 byte vec,
    /// suitable for use as a hash type.
    pub fn to_bytes_36(&self) -> Vec<u8> {
        self.as_u32()
            .to_le_bytes()
            .iter()
            .cycle()
            .take(36)
            .copied()
            .collect()
    }
}
