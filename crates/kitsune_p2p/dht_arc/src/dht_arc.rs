//! A type for indicating ranges on the dht arc

use derive_more::From;
use derive_more::Into;
use num_traits::AsPrimitive;
use std::num::Wrapping;
use std::ops::Bound;
use std::ops::RangeBounds;

#[cfg(test)]
use std::ops::RangeInclusive;

use crate::*;

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

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
/// Represents how much of a dht arc is held
/// start_loc is where the hash is.
/// The start_loc is the left edge of the arc.
/// The length is how far the arc extends to the right.
/// The half-length is half the length.
/// half_length 0 means nothing is held
/// half_length 1 means just the start_loc is held
/// half_length n where n > 1 will hold those locations out to 2n - 1
/// half_length u32::MAX / 2 + 1 covers all locations in the DHT.
///
/// Imagine an array:
/// ```text
///     [0][1][2][3][4][5][6][7]
/// a half length of 3 will give you
///     [0][1][2][3][4]
/// ```
pub struct DhtArc {
    /// The start location of this dht arc
    pub(crate) start_loc: DhtLocation,

    /// The "half-length" of this dht arc
    pub(crate) half_length: u32,
}

impl DhtArc {
    /// Create an Arc from a hash location plus a length on either side
    /// half length is (0..(u32::MAX / 2 + 1))
    pub fn new<I: Into<DhtLocation>>(start_loc: I, half_length: u32) -> Self {
        let half_length = std::cmp::min(half_length, MAX_HALF_LENGTH);
        Self {
            start_loc: start_loc.into(),
            half_length,
        }
    }

    /// Create a full arc from a start location
    pub fn full<I: Into<DhtLocation>>(start_loc: I) -> Self {
        Self::new(start_loc, MAX_HALF_LENGTH)
    }

    /// Create an empty arc from a start location
    pub fn empty<I: Into<DhtLocation>>(start_loc: I) -> Self {
        Self::new(start_loc, 0)
    }

    /// Create an arc with a coverage.
    pub fn with_coverage<I: Into<DhtLocation>>(start_loc: I, coverage: f64) -> Self {
        let coverage = coverage.clamp(0.0, 1.0);
        Self::new(start_loc, (MAX_HALF_LENGTH as f64 * coverage) as u32)
    }

    /// Update the half length based on a PeerView reading.
    /// This will converge on a new target instead of jumping directly
    /// to the new target and is designed to be called at a given rate
    /// with more recent peer views.
    pub fn update_length<V: Into<PeerView>>(&mut self, view: V) {
        self.half_length =
            (MAX_HALF_LENGTH as f64 * view.into().next_coverage(self.coverage())) as u32;
    }

    /// Check if a location is contained in this arc
    pub fn contains<I: Into<DhtLocation>>(&self, other_location: I) -> bool {
        let other_location = other_location.into();
        let do_hold_something = self.half_length != 0;
        let only_hold_self = self.half_length == 1 && self.start_loc == other_location;
        // Add one to convert to "array length" from math distance
        let dist_as_array_len = wrapped_distance(self.start_loc, other_location.0) + 1;
        // Check for any other dist and the special case of the maximum array len
        let within_range = self.half_length > 1 && dist_as_array_len <= self.half_length;
        // Have to hold something and hold ourself or something within range
        do_hold_something && (only_hold_self || within_range)
    }

    pub fn interval(&self) -> ArcInterval {
        let range = self.range();
        match (range.start_bound(), range.end_bound()) {
            (Bound::Excluded(_), Bound::Excluded(_)) => ArcInterval::Empty,
            (Bound::Included(start), Bound::Included(end)) => ArcInterval::new(*start, *end),
            _ => unreachable!(),
        }
    }

    /// Get the range of the arc
    pub fn range(&self) -> ArcRange {
        if self.half_length == 0 {
            ArcRange {
                start: Bound::Excluded(self.start_loc.into()),
                end: Bound::Excluded(self.start_loc.into()),
            }
        } else if self.half_length == 1 {
            ArcRange {
                start: Bound::Included(self.start_loc.into()),
                end: Bound::Included(self.start_loc.into()),
            }
        // In order to make sure the arc covers the full range we need some overlap at the
        // end to account for division rounding.
        } else if self.half_length >= MAX_HALF_LENGTH - 1 {
            ArcRange {
                start: Bound::Included((self.start_loc.0).0),
                end: Bound::Included(
                    (self.start_loc.0 + Wrapping(2) * DhtLocation::from(MAX_HALF_LENGTH).0
                        - Wrapping(1))
                    .0,
                ),
            }
        } else {
            ArcRange {
                start: Bound::Included((self.start_loc.0).0),
                end: Bound::Included((self.start_loc.0 + DhtLocation::from(self.half_length).0).0),
            }
        }
    }

    /// Represent an arc as an optional range of inclusive endpoints.
    /// If none, the arc length is 0
    pub fn primitive_range_grouped(&self) -> Option<(u32, u32)> {
        let ArcRange { start, end } = self.range();
        match (start, end) {
            (Bound::Included(a), Bound::Included(b)) => Some((a, b)),
            (Bound::Excluded(_), Bound::Excluded(_)) => None,
            _ => unreachable!(),
        }
    }

    /// Same as primitive_range, but with the return type "inside-out"
    pub fn primitive_range_detached(&self) -> (Option<u32>, Option<u32>) {
        self.primitive_range_grouped()
            .map(|(a, b)| (Some(a), Some(b)))
            .unwrap_or_default()
    }

    /// The percentage of the full circle that is covered
    /// by this arc.
    pub fn coverage(&self) -> f64 {
        self.half_length as f64 / MAX_HALF_LENGTH as f64
    }

    /// Get the half length of this arc.
    pub fn half_length(&self) -> u32 {
        self.half_length
    }

    /// Get the half length of this arc.
    pub fn half_length_mut(&mut self) -> &mut u32 {
        &mut self.half_length
    }

    /// Get the start location of this arc.
    pub fn start_loc(&self) -> DhtLocation {
        self.start_loc
    }

    /// Is this DhtArc empty?
    pub fn is_empty(&self) -> bool {
        self.half_length == 0
    }
}

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

#[derive(Debug, Clone, Eq, PartialEq)]
/// This represents the range of values covered by an arc
pub struct ArcRange {
    /// The start bound of an arc range
    pub start: Bound<u32>,

    /// The end bound of an arc range
    pub end: Bound<u32>,
}

impl ArcRange {
    /// Show if the bound is empty
    /// Useful before using as an index
    pub fn is_empty(&self) -> bool {
        matches!((self.start_bound(), self.end_bound()), (Bound::Excluded(a), Bound::Excluded(b)) if a == b)
    }

    /// Length of this range. Remember this range can be a wrapping range.
    /// Must be u64 because the length of possible values in a u32 is u32::MAX + 1.
    pub fn len(&self) -> u64 {
        match (self.start_bound(), self.end_bound()) {
            // Range has wrapped around.
            (Bound::Included(start), Bound::Included(end)) if end < start => {
                U32_LEN - *start as u64 + *end as u64 + 1
            }
            (Bound::Included(start), Bound::Included(end)) if start == end => 1,
            (Bound::Included(start), Bound::Included(end)) => (end - start) as u64 + 1,
            (Bound::Excluded(_), Bound::Excluded(_)) => 0,
            _ => unreachable!("Ranges are either completely inclusive or completely exclusive"),
        }
    }

    #[cfg(test)]
    pub(crate) fn into_inc(self: ArcRange) -> RangeInclusive<usize> {
        match self {
            ArcRange {
                start: Bound::Included(a),
                end: Bound::Included(b),
            } if a <= b => RangeInclusive::new(a as usize, b as usize),
            arc => panic!(
                "This range goes all the way around the arc from {:?} to {:?}",
                arc.start_bound(),
                arc.end_bound()
            ),
        }
    }
}

impl RangeBounds<u32> for ArcRange {
    fn start_bound(&self) -> Bound<&u32> {
        match &self.start {
            Bound::Included(i) => Bound::Included(i),
            Bound::Excluded(i) => Bound::Excluded(i),
            Bound::Unbounded => unreachable!("No unbounded ranges for arcs"),
        }
    }

    fn end_bound(&self) -> Bound<&u32> {
        match &self.end {
            Bound::Included(i) => Bound::Included(i),
            Bound::Excluded(i) => Bound::Excluded(i),
            Bound::Unbounded => unreachable!("No unbounded ranges for arcs"),
        }
    }

    fn contains<U>(&self, _item: &U) -> bool
    where
        u32: PartialOrd<U>,
        U: ?Sized + PartialOrd<u32>,
    {
        unimplemented!("Contains doesn't make sense for this type of range due to redundant holding near the bounds. Use DhtArc::contains")
    }
}

impl std::fmt::Display for DhtArc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_ascii(100))
    }
}

impl DhtArc {
    pub fn to_ascii(&self, len: usize) -> String {
        let mut out = vec![" "; len];
        let lenf = len as f64;
        let len = len as isize;
        let half_cov = (self.coverage() * lenf / 2.0) as isize;
        let start = (self.start_loc.0).0 as f64 / U32_LEN as f64;
        let start = (start * lenf) as isize;
        for mut i in start..(start + 2 * half_cov) {
            if i >= len {
                i -= len;
            }
            if i < 0 {
                i += len;
            }
            out[i as usize] = "-";
        }
        out[start as usize] = "@";
        let out: String = out.iter().map(|a| a.chars()).flatten().collect();
        out
    }

    pub fn from_interval(interval: ArcInterval) -> Self {
        match interval.quantized() {
            ArcInterval::Empty => Self::empty(0),
            ArcInterval::Full => Self::full(0),
            ArcInterval::Bounded(start, end) => {
                let start = start.as_u32();
                let end = end.as_u32();
                if start <= end {
                    let half_length = ((end as f64 - start as f64 + 2f64) / 2f64).round() as u32;
                    Self::new(start, half_length)
                } else {
                    let half_length = MAX_HALF_LENGTH - ((start - end) / 2);
                    Self::new(start, half_length)
                }
            }
        }
    }
}

/// Scale a number in a smaller space (specified by `len`) up into the `u32` space.
/// The number to scale can be negative, which is wrapped to a positive value via modulo
#[cfg(any(test, feature = "test_utils"))]
pub(crate) fn loc_upscale(len: usize, v: i32) -> u32 {
    let max = 2f64.powi(32);
    let lenf = len as f64;
    let vf = v as f64;
    (max / lenf * vf) as i64 as u32
}

/// Scale a u32 DhtLocation down into a smaller space (specified by `len`)
#[cfg(any(test, feature = "test_utils"))]
pub(crate) fn loc_downscale(len: usize, d: DhtLocation) -> usize {
    let max = 2f64.powi(32);
    let lenf = len as f64;
    ((lenf / max * (d.as_u32() as f64)) as usize) % len
}

#[test]
fn test_loc_upscale() {
    let m = 2f64.powi(32);
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

#[test]
/// Test ArcInterval -> DhtArc -> ArcInterval roundtrips
/// Note that the intervals must be "quantized" to have an odd length
/// to be representable as DhtArc, so true roundtrips are not possible in general
fn interval_dht_arc_roundtrip() {
    use pretty_assertions::assert_eq;

    // a big number: 1073741823
    const A: u32 = u32::MAX / 4;
    // another big number: 3221225469
    const B: u32 = A * 3;
    // the biggest number: 4294967295
    const M: u32 = u32::MAX;

    let intervals = vec![
        ArcInterval::<u32>::new(0, 0).canonical(),
        ArcInterval::<u32>::new(2, 2).canonical(),
        ArcInterval::<u32>::new(3, 3).canonical(),
        ArcInterval::<u32>::new(3, 5).canonical(),
        ArcInterval::<u32>::new(2, 6).canonical(),
        ArcInterval::<u32>::new(3, 6).canonical(),
        ArcInterval::<u32>::new(3, 7).canonical(),
        ArcInterval::<u32>::new(3, 8).canonical(),
        ArcInterval::<u32>::new(M, M).canonical(),
        ArcInterval::<u32>::new(A, B).canonical(),
        ArcInterval::<u32>::new(B, A).canonical(),
        ArcInterval::<u32>::new(B + 1, A).canonical(),
        ArcInterval::<u32>::new(B - 1, A).canonical(),
        ArcInterval::<u32>::new(B - 1, A + 1).canonical(),
        ArcInterval::<u32>::new(B + 1, A - 1).canonical(),
        ArcInterval::<u32>::new(1, M).canonical(),
        ArcInterval::<u32>::new(2, M).canonical(),
        ArcInterval::<u32>::new(3, M).canonical(),
        ArcInterval::<u32>::new(3, M - 1).canonical(),
        ArcInterval::<u32>::new(3, M - 2).canonical(),
    ];
    // Show that roundtrips of quantized intervals produce no change
    // (roundtrips of unquantized intervals only result in quantization)
    let quantized: Vec<_> = intervals.iter().map(|i| i.quantized()).collect();
    let roundtrips: Vec<_> = intervals
        .iter()
        .map(|i| DhtArc::from_interval(i.to_owned()).interval())
        .collect();

    // Show that roundtrips don't alter the starting points at all
    let original_starts: Vec<_> = intervals.iter().map(|i| i.start_loc()).collect();
    let quantized_starts: Vec<_> = quantized.iter().map(|i| i.start_loc()).collect();
    let roundtrip_starts: Vec<_> = roundtrips.iter().map(|i| i.start_loc()).collect();
    let dht_arc_starts: Vec<_> = quantized
        .iter()
        .map(|i| DhtArc::from_interval(i.clone()).start_loc())
        .collect();
    assert_eq!(quantized, roundtrips);
    assert_eq!(original_starts, quantized_starts);
    assert_eq!(original_starts, roundtrip_starts);
    assert_eq!(original_starts, dht_arc_starts);
}

#[test]
/// Test DhtArc -> ArcInterval -> DhtArc roundtrips
fn dht_arc_interval_roundtrip() {
    use pretty_assertions::assert_eq;

    // ignore empty ArcIntervals, which can't map back to DhtArc
    let arcs = vec![
        // DhtArc::new(1, 0),
        DhtArc::new(1, 1),
        DhtArc::new(1, 2),
        DhtArc::new(1, 3),
        // DhtArc::new(0, 0),
        DhtArc::new(0, 1),
        DhtArc::new(0, 2),
        DhtArc::new(0, 3),
        // DhtArc::new(-1, 0),
        DhtArc::new(-1, 1),
        DhtArc::new(-1, 2),
        DhtArc::new(-1, 3),
        // DhtArc::new(-2, 0),
        DhtArc::new(-2, 1),
        DhtArc::new(-2, 2),
        DhtArc::new(-2, 3),
    ];
    let roundtrips: Vec<_> = arcs
        .iter()
        .map(|arc| DhtArc::from_interval(arc.interval()))
        .collect();
    assert_eq!(arcs, roundtrips);
}
