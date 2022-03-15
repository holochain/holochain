use derive_more::From;
use derive_more::Into;
use num_traits::AsPrimitive;
use std::num::Wrapping;

/// Type for representing a location that can wrap around
/// a u32 dht arc
#[derive(
    Debug,
    Clone,
    Copy,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Into,
    derive_more::AsRef,
    derive_more::Deref,
    derive_more::Display,
)]
pub struct DhtLocation(pub Wrapping<u32>);

impl DhtLocation {
    pub fn new(loc: u32) -> Self {
        Self(Wrapping(loc))
    }

    pub fn as_u32(&self) -> u32 {
        self.0 .0
    }

    pub fn as_i64(&self) -> i64 {
        self.0 .0 as i64
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn as_i32(&self) -> i32 {
        self.0 .0 as i32
    }
}

// This From impl exists to make it easier to construct DhtLocations near the
// maximum value in tests
#[cfg(any(test, feature = "test_utils"))]
impl From<i32> for DhtLocation {
    fn from(i: i32) -> Self {
        (i as u32).into()
    }
}

#[cfg(feature = "sqlite")]
impl rusqlite::ToSql for DhtLocation {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::Owned(self.0 .0.into()))
    }
}

/// The maximum you can hold either side of the hash location
/// is half the circle.
/// This is half of the furthest index you can hold
/// 1 is added for rounding
/// 1 more is added to represent the middle point of an odd length array
pub const MAX_HALF_LENGTH: u32 = (u32::MAX / 2) + 1 + 1;

/// Maximum number of values that a u32 can represent.
pub(crate) const U32_LEN: u64 = u32::MAX as u64 + 1;

impl From<u32> for DhtLocation {
    fn from(a: u32) -> Self {
        Self(Wrapping(a))
    }
}

impl AsPrimitive<u32> for DhtLocation {
    fn as_(self) -> u32 {
        self.as_u32()
    }
}

impl num_traits::Num for DhtLocation {
    type FromStrRadixErr = <u32 as num_traits::Num>::FromStrRadixErr;

    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        u32::from_str_radix(str, radix).map(Self::new)
    }
}

impl std::ops::Add for DhtLocation {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for DhtLocation {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul for DhtLocation {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl std::ops::Div for DhtLocation {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl std::ops::Rem for DhtLocation {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        Self(self.0 % rhs.0)
    }
}

impl num_traits::Zero for DhtLocation {
    fn zero() -> Self {
        Self::new(0)
    }

    fn is_zero(&self) -> bool {
        self.0 .0 == 0
    }
}

impl num_traits::One for DhtLocation {
    fn one() -> Self {
        Self::new(1)
    }
}

impl interval::ops::Width for DhtLocation {
    type Output = u32;

    fn max_value() -> Self {
        u32::max_value().into()
    }

    fn min_value() -> Self {
        u32::min_value().into()
    }

    fn width(lower: &Self, upper: &Self) -> Self::Output {
        u32::width(&lower.0 .0, &upper.0 .0)
    }
}

impl From<DhtLocation> for u32 {
    fn from(l: DhtLocation) -> Self {
        (l.0).0
    }
}

/// Finds the distance from `b` to `a` in a circular space
pub(crate) fn wrapped_distance<A: Into<DhtLocation>, B: Into<DhtLocation>>(a: A, b: B) -> u32 {
    // Turn into wrapped u32s
    let a = a.into().0;
    let b = b.into().0;
    (b - a).0
}

/// Scale a number in a smaller space (specified by `len`) up into the `u32` space.
/// The number to scale can be negative, which is wrapped to a positive value via modulo
#[cfg(any(test, feature = "test_utils"))]
pub(crate) fn loc_upscale(len: usize, v: i32) -> u32 {
    let max = crate::FULL_LEN_F;
    let lenf = len as f64;
    let vf = v as f64;
    (max / lenf * vf) as i64 as u32
}

/// Scale a u32 DhtLocation down into a smaller space (specified by `len`)
#[cfg(any(test, feature = "test_utils"))]
pub(crate) fn loc_downscale(len: usize, d: DhtLocation) -> usize {
    let max = crate::FULL_LEN_F;
    let lenf = len as f64;
    ((lenf / max * (d.as_u32() as f64)) as usize) % len
}

#[test]
fn test_loc_upscale() {
    let m = crate::FULL_LEN_F;
    assert_eq!(loc_upscale(8, 0), DhtLocation::from(0).as_u32());
    assert_eq!(
        loc_upscale(8, 1),
        DhtLocation::from((m / 8.0) as u32).as_u32()
    );
    assert_eq!(
        loc_upscale(3, 1),
        DhtLocation::from((m / 3.0) as u32).as_u32()
    );
}
