use num_traits::Zero;

use crate::hash::{OpHash, RegionHash};

use super::RegionDataConstraints;

/// Take bitwise XOR of each element of both arrays
pub fn array_xor<const N: usize>(a: &mut [u8; N], b: &[u8; N]) {
    for i in 0..N {
        a[i] ^= b[i];
    }
}

/// Take bitwise XOR of each element of both slices
pub fn slice_xor(a: &mut [u8], b: &[u8]) {
    debug_assert_eq!(a.len(), b.len());
    for i in 0..a.len() {
        a[i] ^= b[i];
    }
}

impl RegionHash {
    /// Any null node hashes just get ignored.
    pub fn xor(&mut self, other: &Self) {
        array_xor(&mut *self, other);
    }
}

impl std::ops::Add for RegionHash {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        Self::xor(&mut self, &rhs);
        self
    }
}

impl num_traits::Zero for RegionHash {
    fn zero() -> Self {
        Self::new([0; 32])
    }

    fn is_zero(&self) -> bool {
        *self == Self::zero()
    }
}

impl std::iter::Sum for RegionHash {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|a, b| a + b).unwrap_or_else(RegionHash::zero)
    }
}

impl From<OpHash> for RegionHash {
    fn from(h: OpHash) -> Self {
        Self::new(h.0)
    }
}

/// The pertinent data that we care about for each Region. This is what gets
/// sent over gossip so that nodes can discover which Regions are different
/// between them.
///
/// The size and count data can also act as heuristics to help us fine-tune the
/// gossip algorithm, although currently they are unused (except for the purpose
/// of disambiguation in the rare case of an XOR hash collision).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "RegionDataCompact")]
#[serde(into = "RegionDataCompact")]
pub struct RegionData {
    /// The XOR of hashes of all Ops in this Region
    pub hash: RegionHash,
    /// The total size of Op data contains in this Region
    pub size: u32,
    /// The number of Ops in this Region.
    pub count: u32,
}

impl RegionDataConstraints for RegionData {
    fn count(&self) -> u32 {
        self.count
    }

    fn size(&self) -> u32 {
        self.count
    }
}

impl num_traits::Zero for RegionData {
    fn zero() -> Self {
        Self {
            hash: RegionHash::zero(),
            size: 0,
            count: 0,
        }
    }

    fn is_zero(&self) -> bool {
        if self.count == 0 {
            debug_assert_eq!(self.size, 0);
            debug_assert_eq!(self.hash, RegionHash::zero());
            true
        } else {
            false
        }
    }
}

impl std::ops::AddAssign for RegionData {
    fn add_assign(&mut self, other: Self) {
        // dbg!("add regions", &self, &other);
        self.hash.xor(&other.hash);
        self.size += other.size;
        self.count += other.count;
    }
}

impl std::ops::Add for RegionData {
    type Output = Self;

    fn add(mut self, other: Self) -> Self::Output {
        self += other;
        self
    }
}

impl std::iter::Sum for RegionData {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|a, b| a + b).unwrap_or_else(RegionData::zero)
    }
}

impl std::ops::SubAssign for RegionData {
    fn sub_assign(&mut self, other: Self) {
        // XOR works as both addition and subtraction
        // dbg!("subtract regions", &self, &other);
        self.hash.xor(&other.hash);
        self.size -= other.size;
        self.count -= other.count;
    }
}

impl std::ops::Sub for RegionData {
    type Output = Self;

    fn sub(mut self, other: Self) -> Self::Output {
        self -= other;
        self
    }
}

/// Tuple-based representation of RegionData, used for sending more compact
/// wire messages
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RegionDataCompact(RegionHash, u32, u32);

impl From<RegionData> for RegionDataCompact {
    fn from(d: RegionData) -> Self {
        Self(d.hash, d.size, d.count)
    }
}

impl From<RegionDataCompact> for RegionData {
    fn from(RegionDataCompact(hash, size, count): RegionDataCompact) -> Self {
        Self { hash, size, count }
    }
}

#[test]
fn region_data_is_compact() {
    let hash: RegionHash = crate::hash::fake_hash().into();
    let original = holochain_serialized_bytes::encode(&RegionData {
        hash: hash.clone(),
        size: 1111,
        count: 11,
    })
    .unwrap();
    let compact =
        holochain_serialized_bytes::encode(&RegionDataCompact(hash.clone(), 1111, 11)).unwrap();
    assert_eq!(compact.len(), original.len());
}
