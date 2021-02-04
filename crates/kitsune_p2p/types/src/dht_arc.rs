//! A type for indicating ranges on the dht arc

use derive_more::From;
use derive_more::Into;
use std::num::Wrapping;
use std::ops::Bound;
use std::ops::RangeBounds;
#[cfg(test)]
use std::ops::RangeInclusive;

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
        } else if self.half_length == MAX_HALF_LENGTH {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: This is a really good place for prop testing

    #[test]
    fn test_arc_dist() {
        // start at 5 go all the way around the arc anti-clockwise until
        // you reach 5. You will have traveled 5 less then the entire arc plus one
        // for the reserved zero value
        assert_eq!(shortest_arc_distance(10, 5), 5);
        assert_eq!(shortest_arc_distance(5, 10), 5);
        assert_eq!(
            shortest_arc_distance(Wrapping(u32::MAX) + Wrapping(5), u32::MAX),
            5
        );
        assert_eq!(shortest_arc_distance(0, u32::MAX), 1);
        assert_eq!(
            shortest_arc_distance(0, MAX_HALF_LENGTH),
            MAX_HALF_LENGTH - 2
        );
    }

    #[test]
    fn test_dht_arc() {
        assert!(!DhtArc::new(0, 0).contains(0));

        assert!(DhtArc::new(0, 1).contains(0));

        assert!(!DhtArc::new(0, 1).contains(1));
        assert!(!DhtArc::new(1, 0).contains(0));
        assert!(!DhtArc::new(1, 0).contains(1));

        assert!(DhtArc::new(1, 1).contains(1));
        assert!(DhtArc::new(0, 2).contains(0));
        assert!(DhtArc::new(0, 2).contains(1));
        assert!(DhtArc::new(0, 2).contains(u32::MAX));

        assert!(!DhtArc::new(0, 2).contains(2));
        assert!(!DhtArc::new(0, 2).contains(3));
        assert!(!DhtArc::new(0, 2).contains(u32::MAX - 1));
        assert!(!DhtArc::new(0, 2).contains(u32::MAX - 2));

        assert!(DhtArc::new(0, 3).contains(2));
        assert!(DhtArc::new(0, 3).contains(u32::MAX - 1));
        assert!(DhtArc::new(0, MAX_HALF_LENGTH).contains(u32::MAX / 2));
        assert!(DhtArc::new(0, MAX_HALF_LENGTH).contains(u32::MAX));
        assert!(DhtArc::new(0, MAX_HALF_LENGTH).contains(0));
        assert!(DhtArc::new(0, MAX_HALF_LENGTH).contains(MAX_HALF_LENGTH));
    }

    #[test]
    fn test_arc_start_end() {
        use std::ops::Bound::*;

        let quarter = (u32::MAX as f64 / 4.0).round() as u32;
        let half = (u32::MAX as f64 / 2.0).round() as u32;

        // Checks that the range is contained and the outside of the range isn't contained
        let check_bounds = |mid, hl, start, end| {
            let out_l = (Wrapping(start) - Wrapping(1u32)).0;
            let out_r = (Wrapping(end) + Wrapping(1u32)).0;
            let opp = (Wrapping(mid) + Wrapping(half)).0;

            assert!(!DhtArc::new(mid, hl).contains(out_l));
            assert!(DhtArc::new(mid, hl).contains(start));
            assert!(DhtArc::new(mid, hl).contains(mid));
            assert!(DhtArc::new(mid, hl).contains(end));
            assert!(!DhtArc::new(mid, hl).contains(out_r));
            assert!(!DhtArc::new(mid, hl + 1).contains(opp));
        };

        // Checks that everything is contained because this is a full range
        let check_bounds_full = |mid, hl, start, end| {
            let out_l = (Wrapping(start) - Wrapping(1u32)).0;
            let out_r = (Wrapping(end) + Wrapping(1u32)).0;
            let opp = (Wrapping(mid) + Wrapping(half)).0;

            assert!(DhtArc::new(mid, hl).contains(out_l));
            assert!(DhtArc::new(mid, hl).contains(start));
            assert!(DhtArc::new(mid, hl).contains(mid));
            assert!(DhtArc::new(mid, hl).contains(end));
            assert!(DhtArc::new(mid, hl).contains(out_r));
            assert!(DhtArc::new(mid, hl + 1).contains(opp));
        };

        assert!(DhtArc::new(0, 0).range().is_empty());
        assert_eq!(DhtArc::new(0, 1).range().into_inc(), 0..=0);
        assert_eq!(DhtArc::new(1, 2).range().into_inc(), 0..=2);
        assert_eq!(
            DhtArc::new(quarter, quarter + 1).range().into_inc(),
            0..=(half as usize)
        );
        check_bounds(quarter, quarter + 1, 0, half);

        assert_eq!(
            DhtArc::new(half, quarter + 1).range().into_inc(),
            (quarter as usize)..=((quarter * 3) as usize)
        );
        check_bounds(half, quarter + 1, quarter, quarter * 3);

        assert_eq!(
            DhtArc::new(half, MAX_HALF_LENGTH).range().into_inc(),
            0..=(u32::MAX as usize)
        );
        check_bounds_full(half, MAX_HALF_LENGTH, 0, u32::MAX);

        assert_eq!(
            DhtArc::new(half, MAX_HALF_LENGTH - 2).range().into_inc(),
            2..=((u32::MAX - 1) as usize)
        );
        check_bounds(half, MAX_HALF_LENGTH - 2, 2, u32::MAX - 1);

        assert_eq!(
            DhtArc::new(0, 2).range(),
            ArcRange {
                start: Included(u32::MAX),
                end: Included(1)
            }
        );
        check_bounds(0, 2, u32::MAX, 1);

        assert_eq!(
            DhtArc::new(u32::MAX, 2).range(),
            ArcRange {
                start: Included(u32::MAX - 1),
                end: Included(0)
            }
        );
        check_bounds(u32::MAX, 2, u32::MAX - 1, 0);

        assert_eq!(
            DhtArc::new(0, MAX_HALF_LENGTH).range(),
            ArcRange {
                start: Included(half),
                end: Included(half - 1)
            }
        );
        check_bounds_full(0, MAX_HALF_LENGTH, half, half - 1);
    }
}
