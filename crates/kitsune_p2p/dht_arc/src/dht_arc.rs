//! A type for indicating ranges on the dht arc

use derive_more::From;
use derive_more::Into;
use num_traits::AsPrimitive;
use std::num::Wrapping;
use std::ops::Bound;
use std::ops::RangeBounds;

#[cfg(test)]
use std::ops::RangeInclusive;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_ascii;

mod dht_arc_set;
pub use dht_arc_set::{ArcInterval, DhtArcSet};

mod dht_arc_bucket;
pub use dht_arc_bucket::*;

#[cfg(any(test, feature = "test_utils"))]
pub mod gaps;

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
const U32_LEN: u64 = u32::MAX as u64 + 1;

/// Number of copies of a given hash available at any given time.
const REDUNDANCY_TARGET: usize = 50;

/// If the redundancy drops due to inaccurate estimation we can't
/// go lower then this level of redundancy.
/// Note this can only be tested and not proved.
const REDUNDANCY_FLOOR: usize = 20;

/// Default assumed up time for nodes.
const DEFAULT_UPTIME: f64 = 0.5;

/// The minimum number of peers before sharding can begin.
/// This factors in the expected uptime to reach the redundancy target.
pub const MIN_PEERS: usize = (REDUNDANCY_TARGET as f64 / DEFAULT_UPTIME) as usize;

/// The minimum number of peers we can consider acceptable to see in our arc
/// during testing.
pub const MIN_REDUNDANCY: usize = (REDUNDANCY_FLOOR as f64 / DEFAULT_UPTIME) as usize;

/// The amount "change in arc" is scaled to prevent rapid changes.
/// This also represents the maximum coverage change in a single update
/// as a difference of 1.0 would scale to 0.2.
const DELTA_SCALE: f64 = 0.2;

/// The minimal "change in arc" before we stop scaling.
/// This prevents never reaching the target arc coverage.
const DELTA_THRESHOLD: f64 = 0.01;

/// Due to estimation noise we don't want a very small difference
/// between observed coverage and estimated coverage to
/// amplify when scaled to by the estimated total peers.
/// This threshold must be reached before an estimated coverage gap
/// is calculated.
const NOISE_THRESHOLD: f64 = 0.01;

// TODO: Use the [`f64::clamp`] when we switch to rustc 1.50
fn clamp(min: f64, max: f64, mut x: f64) -> f64 {
    if x < min {
        x = min;
    }
    if x > max {
        x = max;
    }
    x
}

/// The ideal coverage if all peers were holding the same sized
/// arcs and our estimated total peers is close.
fn coverage_target(est_total_peers: usize) -> f64 {
    if est_total_peers <= REDUNDANCY_TARGET {
        1.0
    } else {
        REDUNDANCY_TARGET as f64 / est_total_peers as f64
    }
}

/// Calculate the target arc length given a peer density.
fn target(density: PeerDensity) -> f64 {
    // Get the estimated coverage gap based on our observed peer density.
    let est_gap = density.est_gap();
    // If we haven't observed at least our redundancy target number
    // of peers (adjusted for expected uptime) then we know that the data
    // in our arc is under replicated and we should start aiming for full coverage.
    if density.expected_count() < REDUNDANCY_TARGET {
        1.0
    } else {
        // Get the estimated gap. We don't care about negative gaps
        // or gaps we can't fill (> 1.0)
        let est_gap = clamp(0.0, 1.0, est_gap);
        // Get the ideal coverage target for the size of that we estimate
        // the network to be.
        let ideal_target = coverage_target(density.est_total_peers());
        // Take whichever is larger. We prefer nodes to target the ideal
        // coverage but if there is a larger gap then it needs to be filled.
        let target = est_gap.max(ideal_target);

        clamp(0.0, 1.0, target)
    }
}

/// The convergence algorithm that moves an arc towards
/// our estimated target.
///
/// Note the rate of convergence is dependant of the rate
/// that [`DhtArc::update_length`] is called.
fn converge(current: f64, density: PeerDensity) -> f64 {
    let target = target(density);
    // The change in arc we'd need to make to get to the target.
    let delta = target - current;
    // If this is below our threshold then apply that delta.
    if delta.abs() < DELTA_THRESHOLD {
        current + delta
    // Other wise scale the delta to avoid rapid change.
    } else {
        current + (delta * DELTA_SCALE)
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
/// Represents how much of a dht arc is held
/// center_loc is where the hash is.
/// The center_loc is the center of the arc
/// The half length is the length of items held
/// from the center in both directions
/// half_length 0 means nothing is held
/// half_length 1 means just the center_loc is held
/// half_length n where n > 1 will hold those positions out
/// half_length u32::MAX / 2 + 1 covers all positions
/// on either side of center_loc.
/// Imagine an bidirectional array:
/// ```text
/// [4][3][2][1][0][1][2][3][4]
// half length of 3 will give you
///       [2][1][0][1][2]
/// ```
pub struct DhtArc {
    /// The center location of this dht arc
    center_loc: DhtLocation,

    /// The "half-length" of this dht arc
    half_length: u32,
}

impl DhtArc {
    /// Create an Arc from a hash location plus a length on either side
    /// half length is (0..(u32::Max / 2 + 1))
    pub fn new<I: Into<DhtLocation>>(center_loc: I, half_length: u32) -> Self {
        let half_length = std::cmp::min(half_length, MAX_HALF_LENGTH);
        Self {
            center_loc: center_loc.into(),
            half_length,
        }
    }

    /// Create a full arc from a center location
    pub fn full<I: Into<DhtLocation>>(center_loc: I) -> Self {
        Self::new(center_loc, MAX_HALF_LENGTH)
    }

    /// Create an empty arc from a center location
    pub fn empty<I: Into<DhtLocation>>(center_loc: I) -> Self {
        Self::new(center_loc, 0)
    }

    /// Create an arc with a coverage.
    pub fn with_coverage<I: Into<DhtLocation>>(center_loc: I, coverage: f64) -> Self {
        let coverage = coverage.clamp(0.0, 1.0);
        Self::new(center_loc, (MAX_HALF_LENGTH as f64 * coverage) as u32)
    }

    /// Update the half length based on a density reading.
    /// This will converge on a new target instead of jumping directly
    /// to the new target and is designed to be called at a given rate
    /// with more recent peer density readings.
    pub fn update_length(&mut self, density: PeerDensity) {
        self.half_length = (MAX_HALF_LENGTH as f64 * converge(self.coverage(), density)) as u32;
    }

    /// Check if a location is contained in this arc
    pub fn contains<I: Into<DhtLocation>>(&self, other_location: I) -> bool {
        let other_location = other_location.into();
        let do_hold_something = self.half_length != 0;
        let only_hold_self = self.half_length == 1 && self.center_loc == other_location;
        // Add one to convert to "array length" from math distance
        let dist_as_array_len = shortest_arc_distance(self.center_loc, other_location.0) + 1;
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
                start: Bound::Excluded(self.center_loc.into()),
                end: Bound::Excluded(self.center_loc.into()),
            }
        } else if self.half_length == 1 {
            ArcRange {
                start: Bound::Included(self.center_loc.into()),
                end: Bound::Included(self.center_loc.into()),
            }
        // In order to make sure the arc covers the full range we need some overlap at the
        // end to account for division rounding.
        } else if self.half_length >= MAX_HALF_LENGTH - 1 {
            ArcRange {
                start: Bound::Included(
                    (self.center_loc.0 - DhtLocation::from(MAX_HALF_LENGTH - 1).0).0,
                ),
                end: Bound::Included(
                    (self.center_loc.0 + DhtLocation::from(MAX_HALF_LENGTH).0 - Wrapping(2)).0,
                ),
            }
        } else {
            ArcRange {
                start: Bound::Included(
                    (self.center_loc.0 - DhtLocation::from(self.half_length - 1).0).0,
                ),
                end: Bound::Included(
                    (self.center_loc.0 + DhtLocation::from(self.half_length).0 - Wrapping(1)).0,
                ),
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

    /// The absolute length that this arc will hold.
    pub fn absolute_length(&self) -> u64 {
        self.range().len()
    }

    /// The percentage of the full circle that is covered
    /// by this arc.
    pub fn coverage(&self) -> f64 {
        self.absolute_length() as f64 / U32_LEN as f64
    }

    /// Get the half length of this arc.
    pub fn half_length(&self) -> u32 {
        self.half_length
    }

    /// Get the center location of this arc.
    pub fn center_loc(&self) -> DhtLocation {
        self.center_loc
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

/// Finds the shortest distance between two points on a circle
fn shortest_arc_distance<A: Into<DhtLocation>, B: Into<DhtLocation>>(a: A, b: B) -> u32 {
    // Turn into wrapped u32s
    let a = a.into().0;
    let b = b.into().0;
    std::cmp::min(a - b, b - a).0
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
    fn into_inc(self: ArcRange) -> RangeInclusive<usize> {
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
        let mut out = ["_"; 100];
        let half_cov = (self.coverage() * 50.0) as isize;
        let center = self.center_loc.0 .0 as f64 / U32_LEN as f64;
        let center = (center * 100.0) as isize;
        for mut i in (center - half_cov)..(center + half_cov) {
            if i >= 100 {
                i -= 100;
            }
            if i < 0 {
                i += 100;
            }
            out[i as usize] = "#";
        }
        out[center as usize] = "|";
        let out: String = out.iter().map(|a| a.chars()).flatten().collect();
        writeln!(f, "[{}]", out)
    }
}

impl DhtArc {
    pub fn from_interval(interval: ArcInterval) -> Self {
        match interval.quantized() {
            ArcInterval::Empty => Self::empty(0),
            ArcInterval::Full => Self::full(0),
            ArcInterval::Bounded(start, end) => {
                let start = start.as_u32();
                let end = end.as_u32();
                if start <= end {
                    let half_length = ((end as f64 - start as f64 + 2f64) / 2f64).round() as u32;
                    let center = ((start as f64 + end as f64) / 2f64).round() as u32;
                    Self::new(center, half_length)
                } else {
                    let half_length = MAX_HALF_LENGTH - ((start - end) / 2);
                    let center = Wrapping(start) + Wrapping(half_length) - Wrapping(1);
                    Self::new(center.0, half_length)
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
/// Test the center_loc calculation for a variety of ArcIntervals
fn arc_interval_center_loc() {
    use pretty_assertions::assert_eq;

    let intervals = vec![
        // singleton
        (ArcInterval::<i32>::new(0, 0), 0),
        (ArcInterval::<i32>::new(2, 2), 2),
        (ArcInterval::<i32>::new(3, 3), 3),
        // non-wrapping
        (ArcInterval::<i32>::new(2, 4), 3),
        (ArcInterval::<i32>::new(2, 5), 4),
        (ArcInterval::<i32>::new(2, 6), 4),
        (ArcInterval::<i32>::new(0, 8), 4),
        (ArcInterval::<i32>::new(1, 8), 5),
        (ArcInterval::<i32>::new(2, 8), 5),
        (ArcInterval::<i32>::new(3, 5), 4),
        (ArcInterval::<i32>::new(3, 6), 5),
        (ArcInterval::<i32>::new(3, 7), 5),
        // wrapping
        (ArcInterval::<i32>::new(-3, 3), 0),
        (ArcInterval::<i32>::new(-4, 3), 0),
        (ArcInterval::<i32>::new(-4, 4), 0),
        (ArcInterval::<i32>::new(-5, 4), 0),
        (ArcInterval::<i32>::new(-4, 2), -1i32),
        (ArcInterval::<i32>::new(-5, 2), -1i32),
        (ArcInterval::<i32>::new(-5, 3), -1i32),
    ];
    let expected: Vec<_> = intervals
        .iter()
        .map(|(_, c)| DhtLocation::from(*c))
        .collect();
    let actual: Vec<_> = intervals
        .into_iter()
        .map(|(i, _)| i.canonical().center_loc())
        .collect();
    assert_eq!(expected, actual);
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

    // Show that roundtrips don't alter the centerpoints at all
    let original_centers: Vec<_> = intervals.iter().map(|i| i.center_loc()).collect();
    let quantized_centers: Vec<_> = quantized.iter().map(|i| i.center_loc()).collect();
    let roundtrip_centers: Vec<_> = roundtrips.iter().map(|i| i.center_loc()).collect();
    let dht_arc_centers: Vec<_> = quantized
        .iter()
        .map(|i| DhtArc::from_interval(i.clone()).center_loc())
        .collect();
    assert_eq!(quantized, roundtrips);
    assert_eq!(original_centers, quantized_centers);
    assert_eq!(original_centers, roundtrip_centers);
    assert_eq!(original_centers, dht_arc_centers);
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
