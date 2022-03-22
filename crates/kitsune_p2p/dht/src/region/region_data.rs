use crate::hash::{OpHash, RegionHash};

pub fn array_xor<const N: usize>(a: &mut [u8; N], b: &[u8; N]) {
    for i in 0..N {
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

impl From<OpHash> for RegionHash {
    fn from(h: OpHash) -> Self {
        Self::new(h.0)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RegionData {
    pub hash: RegionHash,
    pub size: u32,
    pub count: u32,
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
