use crate::DhtLocation;

const F: u32 = 16777216;

pub type Loc8 = i8;

impl From<Loc8> for DhtLocation {
    fn from(i: Loc8) -> Self {
        DhtLocation::new(i as u8 as u32 * F)
    }
}

impl From<DhtLocation> for Loc8 {
    fn from(loc: DhtLocation) -> Self {
        (loc.as_u32() / F) as u8 as i8
    }
}

impl DhtLocation {
    /// Turn this location into a "canonical" 36 byte vec,
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
