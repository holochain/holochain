use std::collections::BTreeSet;

use crate::dht_arc::{loc_downscale, loc_upscale, ArcInterval, DhtLocation};

/// A representation of DhtLocation in the u8 space. Useful for writing tests
/// that test the full range of possible locations while still working with small numbers.
/// A Loc8 can be constructed `From<i8>` within `-128 <= n <= 255`.
/// A negative number is wrapped to a positive number internally, and the `sign` is preserved
/// for display purposes.
///
/// Loc8 has custom `Eq`, `Ord`, and other impls which disregard the `sign`.
#[derive(Copy, Clone)]
pub struct Loc8 {
    /// The unsigned value
    val: u8,
    /// Designates whether this value was constructed with a negative number or not,
    /// so that it can be displayed as positive or negative accordingly.
    sign: bool,
}

impl From<i32> for Loc8 {
    fn from(i: i32) -> Self {
        if i >= 0 {
            Self {
                val: i as u8,
                sign: false,
            }
        } else {
            Self {
                val: i as i8 as u8,
                sign: true,
            }
        }
    }
}

impl PartialEq for Loc8 {
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}

impl Eq for Loc8 {}

impl PartialOrd for Loc8 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.val.partial_cmp(&other.val)
    }
}

impl Ord for Loc8 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.val.cmp(&other.val)
    }
}

impl std::hash::Hash for Loc8 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.val.hash(state);
    }
}

impl std::fmt::Display for Loc8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_i32().fmt(f)
    }
}

impl std::fmt::Debug for Loc8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_i32().fmt(f)
    }
}

impl Loc8 {
    pub fn as_i8(&self) -> i8 {
        self.as_u8() as i8
    }

    pub fn as_u8(&self) -> u8 {
        self.val
    }

    pub fn as_i32(&self) -> i32 {
        if self.sign {
            self.as_i8() as i32
        } else {
            self.as_u8() as u32 as i32
        }
    }

    pub fn set<L: Into<Loc8>, I: IntoIterator<Item = L>>(it: I) -> BTreeSet<Self> {
        it.into_iter().map(Into::into).collect()
    }

    pub fn upscale<L: Into<Loc8>>(v: L) -> u32 {
        let v: Loc8 = v.into();
        loc_upscale(256, v.as_i32())
    }

    pub fn downscale(v: u32) -> u8 {
        loc_downscale(256, DhtLocation::from(v)) as u8
    }
}

impl From<Loc8> for DhtLocation {
    fn from(i: Loc8) -> Self {
        DhtLocation::from(Loc8::upscale(i))
    }
}

impl DhtLocation {
    pub fn as_loc8(&self) -> Loc8 {
        Loc8 {
            val: Loc8::downscale(self.as_u32()),
            sign: false,
        }
    }

    /// Turn this location into a "representative" 36 byte vec,
    /// suitable for use as a hash type.
    #[cfg(feature = "test_utils")]
    pub fn to_representative_test_bytes_36(&self) -> Vec<u8> {
        self.as_u32()
            .to_le_bytes()
            .iter()
            .cycle()
            .take(36)
            .copied()
            .collect()
    }
}

impl ArcInterval {
    pub fn as_loc8(&self) -> ArcInterval<Loc8> {
        match self {
            Self::Empty => ArcInterval::Empty,
            Self::Full => ArcInterval::Full,
            Self::Bounded(lo, hi) => ArcInterval::Bounded(lo.as_loc8(), hi.as_loc8()),
        }
    }
}

impl<L> ArcInterval<L>
where
    Loc8: From<L>,
{
    pub fn canonical(self) -> ArcInterval {
        match self {
            ArcInterval::Empty => ArcInterval::Empty,
            ArcInterval::Full => ArcInterval::Full,
            ArcInterval::Bounded(lo, hi) => ArcInterval::new(
                DhtLocation::from(Loc8::from(lo)),
                DhtLocation::from(Loc8::from(hi)),
            ),
        }
    }
}

#[test]
fn scaling() {
    let f = 16777216i32;
    assert_eq!(Loc8::upscale(4) as i32, f * 4);
    assert_eq!(Loc8::upscale(-4) as i32, f * -4);

    assert_eq!(Loc8::downscale((f * 4) as u32), 4);
    assert_eq!(Loc8::downscale((f * -4) as u32) as i8, -4);
}
