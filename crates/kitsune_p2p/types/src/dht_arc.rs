//! A type for indicating ranges on the dht arc

use derive_more::From;
use derive_more::Into;
use std::num::Wrapping;
use std::ops::Bound;
use std::ops::RangeBounds;

#[cfg(test)]
use std::ops::RangeInclusive;

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test_utils"))]
pub mod gaps;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, From, Into)]
/// Type for representing a location that can wrap around
/// a u32 dht arc
pub struct DhtLocation(pub Wrapping<u32>);

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

#[cfg(any(test, feature = "test_utils"))]
/// Margin of error for floating point comparisons
const ERROR_MARGIN: f64 = 0.0000000001;

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
/// [2][1][0][1][2]
/// ```
pub struct DhtArc {
    /// The center location of this dht arc
    pub center_loc: DhtLocation,

    /// The "half-length" of this dht arc
    pub half_length: u32,
}

#[derive(Debug, Clone, Copy)]
/// The average density of peers at a location in the u32 space.
pub struct PeerDensity {
    /// The arc that filtered the bucket that generated this density.
    filter: DhtArc,
    /// The average coverage of peers in the bucket.
    average_coverage: f64,
    /// The number of peers in the bucket.
    count: usize,
}

/// When sampling a section of the arc we can
/// collect all the other peer [`DhtArc`]s into a
/// DhtBucket.
/// All the peer arcs arc contained within the buckets filter arc.
/// The filter is this peer's "view" into their section of the dht arc.
pub struct DhtArcBucket {
    /// The arc used to filter this bucket.
    filter: DhtArc,
    /// The arcs in this bucket.
    arcs: Vec<DhtArc>,
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
        } else if self.half_length == MAX_HALF_LENGTH || self.half_length == MAX_HALF_LENGTH - 1 {
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
    pub fn primitive_range(&self) -> Option<(u32, u32)> {
        let ArcRange { start, end } = self.range();
        match (start, end) {
            (Bound::Included(a), Bound::Included(b)) => Some((a, b)),
            (Bound::Excluded(_), Bound::Excluded(_)) => None,
            _ => unreachable!(),
        }
    }

    /// Same as primitive_range, but "inside-out"
    pub fn primitive_range_transposed(&self) -> (Option<u32>, Option<u32>) {
        self.primitive_range()
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
}

impl PeerDensity {
    /// Create a new peer density reading from the:
    /// - The filter used to create the bucket.
    /// - Average coverage of all peers in the bucket.
    /// - Count of peers in the bucket.
    pub fn new(filter: DhtArc, average_coverage: f64, count: usize) -> Self {
        Self {
            filter,
            average_coverage,
            count,
        }
    }

    /// The expected number of peers for this arc over time.
    pub fn expected_count(&self) -> usize {
        (self.count as f64 * DEFAULT_UPTIME) as usize
    }

    /// Estimate the gap in coverage that needs to be filled.
    /// If the gap is negative that means we are over covered.
    pub fn est_gap(&self) -> f64 {
        let est_total_peers = self.est_total_peers();
        let ideal_target = coverage_target(est_total_peers);
        let gap = ideal_target - self.average_coverage;
        // We want to check the ratio between the gap and the target
        // because small targets will have small gaps.
        let gap_ratio = gap.abs() / ideal_target;
        if gap_ratio < NOISE_THRESHOLD {
            0.0
        } else {
            gap * est_total_peers as f64
        }
    }

    /// Estimate total peers.
    pub fn est_total_peers(&self) -> usize {
        let coverage = self.filter.coverage();
        if coverage > 0.0 {
            (1.0 / coverage * self.expected_count() as f64) as usize
        } else {
            // If we had no coverage when we collected these
            // peers then we can't make a good guess at the total.
            0
        }
    }

    /// Estimated total redundant coverage.
    pub fn est_total_redundancy(&self) -> usize {
        (self.est_total_peers() as f64 * self.average_coverage) as usize
    }
}

impl From<u32> for DhtLocation {
    fn from(a: u32) -> Self {
        Self(Wrapping(a))
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

impl DhtArcBucket {
    /// Select only the arcs that fit into the bucket.
    pub fn new<I: IntoIterator<Item = DhtArc>>(filter: DhtArc, arcs: I) -> Self {
        let arcs = arcs
            .into_iter()
            .filter(|a| filter.contains(a.center_loc))
            .collect();
        Self { filter, arcs }
    }

    /// Same as new but doesn't check if arcs fit into the bucket.
    pub fn new_unchecked(bucket: DhtArc, arcs: Vec<DhtArc>) -> Self {
        Self {
            filter: bucket,
            arcs,
        }
    }

    /// Get the density of this bucket.
    pub fn density(&self) -> PeerDensity {
        let (total, count) = self
            .arcs
            .iter()
            .fold((0u64, 0usize), |(total, count), arc| {
                (total + arc.half_length as u64, count + 1)
            });
        let average = if count > 0 {
            (total as f64 / count as f64) / MAX_HALF_LENGTH as f64
        } else {
            0.0
        };
        PeerDensity::new(self.filter, average, count)
    }
}

impl std::fmt::Display for DhtArcBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for a in &self.arcs {
            writeln!(f, "{}", a)?;
        }
        writeln!(f, "{} <- Bucket arc", self.filter)
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
