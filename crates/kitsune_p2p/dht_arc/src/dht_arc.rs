//! A type for indicating ranges on the dht arc

use std::ops::Bound;
use std::ops::RangeBounds;

use crate::*;

pub const FULL_LEN: u64 = 2u64.pow(32);
pub const FULL_LEN_F: f64 = FULL_LEN as f64;

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
        unimplemented!("Contains doesn't make sense for this type of range due to redundant holding near the bounds. Use DhtArcRange::contains")
    }
}

/// The main DHT arc type. Represents an Agent's storage Arc on the DHT,
/// preserving the agent's DhtLocation even in the case of a Full or Empty arc.
/// Contrast to [`DhtArcRange`], which is used for cases where the arc is not
/// associated with any particular Agent, and so the agent's Location cannot be known.
#[derive(Copy, Clone, Debug, derive_more::Deref, serde::Serialize, serde::Deserialize)]
pub struct DhtArc(#[deref] DhtArcRange, Option<DhtLocation>);

impl DhtArc {
    pub fn bounded(a: DhtArcRange) -> Self {
        Self(a, None)
    }

    pub fn empty(loc: DhtLocation) -> Self {
        Self(DhtArcRange::Empty, Some(loc))
    }

    pub fn full(loc: DhtLocation) -> Self {
        Self(DhtArcRange::Full, Some(loc))
    }

    /// Create a arc range from a start location with a percentage of
    /// the total coverage.
    pub fn with_coverage(start: DhtLocation, coverage: f64) -> Self {
        Self::from_parts(DhtArcRange::with_coverage(start, coverage), start)
    }

    pub fn start_loc(&self) -> DhtLocation {
        match (self.0, self.1) {
            (DhtArcRange::Empty, Some(loc)) => loc,
            (DhtArcRange::Full, Some(loc)) => loc,
            (DhtArcRange::Bounded(lo, _), _) => lo,
            _ => unreachable!(),
        }
    }

    /// Update the half length based on a PeerView reading.
    /// This will converge on a new target instead of jumping directly
    /// to the new target and is designed to be called at a given rate
    /// with more recent peer views.
    pub fn update_length<V: Into<PeerView>>(&mut self, view: V) {
        let new_length = (U32_LEN as f64 * view.into().next_coverage(self.coverage())) as u64;
        *self = Self::from_start_and_len(self.start_loc(), new_length)
    }

    pub fn inner(self) -> DhtArcRange {
        self.0
    }

    /// Construct from an arc range and a location.
    /// The location is only used if the arc range is full or empty.
    pub fn from_parts(a: DhtArcRange, loc: DhtLocation) -> Self {
        if a.is_bounded() {
            Self::bounded(a)
        } else {
            Self(a, Some(loc))
        }
    }

    pub fn from_start_and_half_len<L: Into<DhtLocation>>(start: L, half_len: u32) -> Self {
        let start = start.into();
        let a = DhtArcRange::from_start_and_half_len(start, half_len);
        Self::from_parts(a, start)
    }

    pub fn from_start_and_len<L: Into<DhtLocation>>(start: L, len: u64) -> Self {
        let start = start.into();
        let a = DhtArcRange::from_start_and_len(start, len);
        Self::from_parts(a, start)
    }

    pub fn from_bounds<L: Into<DhtLocation>>(start: L, end: L) -> Self {
        let start = start.into();
        let end = end.into();
        let a = DhtArcRange::from_bounds(start, end);
        Self::from_parts(a, start)
    }

    /// Get the range of the arc
    pub fn range(&self) -> ArcRange {
        match (self.0, self.1) {
            (DhtArcRange::Empty, Some(loc)) => ArcRange {
                start: Bound::Excluded(loc.as_u32()),
                end: Bound::Excluded(loc.as_u32()),
            },
            (DhtArcRange::Full, Some(loc)) => ArcRange {
                start: Bound::Included(loc.as_u32()),
                end: Bound::Included(loc.as_u32().wrapping_sub(1)),
            },
            (DhtArcRange::Bounded(lo, hi), _) => ArcRange {
                start: Bound::Included(lo.as_u32()),
                end: Bound::Included(hi.as_u32()),
            },
            _ => unimplemented!(),
        }
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn to_ascii(&self, len: usize) -> String {
        let mut s = self.0.to_ascii(len);
        let start = loc_downscale(len, self.start_loc());
        s.replace_range(start..start + 1, "@");
        s
    }
}

impl From<DhtArc> for DhtArcRange {
    fn from(a: DhtArc) -> Self {
        a.inner()
    }
}

impl From<&DhtArc> for DhtArcRange {
    fn from(a: &DhtArc) -> Self {
        a.inner()
    }
}

/// A variant of DHT arc which is intentionally forgetful of the Agent's location.
/// This type is used in places where set logic (union and intersection)
/// is performed on arcs, which splits and joins arcs in such a way that it
/// doesn't make sense to claim that the arc belongs to any particular agent or
/// location.
///
/// This type exists to make sure we don't accidentally intepret the starting
/// point of such a "derived" arc as a legitimate agent location.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DhtArcRange<T = DhtLocation> {
    Empty,
    Full,
    Bounded(T, T),
}

impl<T: PartialOrd + num_traits::Num> DhtArcRange<T> {
    pub fn contains<B: std::borrow::Borrow<T>>(&self, t: B) -> bool {
        match self {
            Self::Empty => false,
            Self::Full => true,
            Self::Bounded(lo, hi) => {
                let t = t.borrow();
                if lo <= hi {
                    lo <= t && t <= hi
                } else {
                    lo <= t || t <= hi
                }
            }
        }
    }
}

impl<T> DhtArcRange<T> {
    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> DhtArcRange<U> {
        match self {
            Self::Empty => DhtArcRange::Empty,
            Self::Full => DhtArcRange::Full,
            Self::Bounded(lo, hi) => DhtArcRange::Bounded(f(lo), f(hi)),
        }
    }

    #[deprecated = "left over from refactor"]
    pub fn interval(self) -> Self {
        self
    }
}

impl<T: num_traits::AsPrimitive<u32>> DhtArcRange<T> {
    pub fn from_bounds(start: T, end: T) -> DhtArcRange<DhtLocation> {
        let start = start.as_();
        let end = end.as_();
        if is_full(start, end) {
            DhtArcRange::Full
        } else {
            DhtArcRange::Bounded(DhtLocation::new(start), DhtLocation::new(end))
        }
    }

    pub fn from_start_and_len(start: T, len: u64) -> DhtArcRange<DhtLocation> {
        let start = start.as_();
        if len == 0 {
            DhtArcRange::Empty
        } else {
            let end = start.wrapping_add(((len - 1) as u32).min(u32::MAX));
            DhtArcRange::from_bounds(start, end)
        }
    }

    /// Convenience for our legacy code which defined arcs in terms of half-lengths
    /// rather than full lengths
    pub fn from_start_and_half_len(start: T, half_len: u32) -> DhtArcRange<DhtLocation> {
        Self::from_start_and_len(start, half_to_full_len(half_len))
    }

    pub fn new_generic(start: T, end: T) -> Self {
        if is_full(start.as_(), end.as_()) {
            Self::Full
        } else {
            Self::Bounded(start, end)
        }
    }
}

impl DhtArcRange<u32> {
    pub fn canonical(self) -> DhtArcRange {
        match self {
            DhtArcRange::Empty => DhtArcRange::Empty,
            DhtArcRange::Full => DhtArcRange::Full,
            DhtArcRange::Bounded(lo, hi) => {
                DhtArcRange::from_bounds(DhtLocation::new(lo), DhtLocation::new(hi))
            }
        }
    }
}

impl DhtArcRange<DhtLocation> {
    /// Constructor
    pub fn new_empty() -> Self {
        Self::Empty
    }

    /// Create a arc range from a start location with a percentage of
    /// the total coverage.
    pub fn with_coverage(start: DhtLocation, coverage: f64) -> Self {
        let coverage = coverage.clamp(0.0, 1.0);
        if coverage == 0.0 {
            Self::Empty
        } else {
            let len = (u32::MAX as f64 * coverage) as u64;
            Self::from_start_and_len(start, len)
        }
    }

    /// Represent an arc as an optional range of inclusive endpoints.
    /// If none, the arc length is 0
    pub fn to_bounds_grouped(&self) -> Option<(DhtLocation, DhtLocation)> {
        match self {
            Self::Empty => None,
            Self::Full => Some((DhtLocation::MIN, DhtLocation::MAX)),
            &Self::Bounded(lo, hi) => Some((lo, hi)),
        }
    }

    /// Same as to_bounds_grouped, but with the return type "inside-out"
    pub fn to_primitive_bounds_detached(&self) -> (Option<u32>, Option<u32>) {
        self.to_bounds_grouped()
            .map(|(a, b)| (Some(a.as_u32()), Some(b.as_u32())))
            .unwrap_or_default()
    }

    /// Check if this arc is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Check if this arc is full.
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }

    /// Check if this arc is bounded.
    pub fn is_bounded(&self) -> bool {
        matches!(self, Self::Bounded(_, _))
    }

    /// Check if arcs overlap
    pub fn overlaps(&self, other: &Self) -> bool {
        let a = DhtArcSet::from(self);
        let b = DhtArcSet::from(other);
        a.overlap(&b)
    }

    /// Amount of intersection between two arcs
    pub fn overlap_coverage(&self, other: &Self) -> f64 {
        let a = DhtArcSet::from(self);
        let b = DhtArcSet::from(other);
        let c = a.intersection(&b);
        c.size() as f64 / a.size() as f64
    }

    /// The percentage of the full circle that is covered
    /// by this arc.
    pub fn coverage(&self) -> f64 {
        self.length() as f64 / 2f64.powf(32.0)
    }

    pub fn length(&self) -> u64 {
        match self {
            DhtArcRange::Empty => 0,
            DhtArcRange::Full => 2u64.pow(32),
            DhtArcRange::Bounded(lo, hi) => {
                (hi.as_u32().wrapping_sub(lo.as_u32()) as u64).wrapping_add(1)
            }
        }
    }

    // #[deprecated = "leftover from refactor"]
    pub fn half_length(&self) -> u32 {
        full_to_half_len(self.length())
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Handy ascii representation of an arc, especially useful when
    /// looking at several arcs at once to get a sense of their overlap
    pub fn to_ascii(&self, len: usize) -> String {
        let empty = || " ".repeat(len);
        let full = || "-".repeat(len);

        // If lo and hi are less than one bucket's width apart when scaled down,
        // decide whether to interpret this as empty or full
        let decide = |lo: &DhtLocation, hi: &DhtLocation| {
            let mid = loc_upscale(len, (len / 2) as i32);
            if lo < hi {
                if hi.as_u32() - lo.as_u32() < mid {
                    empty()
                } else {
                    full()
                }
            } else if lo.as_u32() - hi.as_u32() < mid {
                full()
            } else {
                empty()
            }
        };

        match self {
            Self::Full => full(),
            Self::Empty => empty(),
            Self::Bounded(lo0, hi0) => {
                let lo = loc_downscale(len, *lo0);
                let hi = loc_downscale(len, *hi0);
                if lo0 <= hi0 {
                    if lo >= hi {
                        vec![decide(lo0, hi0)]
                    } else {
                        vec![
                            " ".repeat(lo),
                            "-".repeat(hi - lo + 1),
                            " ".repeat((len - hi).saturating_sub(1)),
                        ]
                    }
                } else if lo <= hi {
                    vec![decide(lo0, hi0)]
                } else {
                    vec![
                        "-".repeat(hi + 1),
                        " ".repeat((lo - hi).saturating_sub(1)),
                        "-".repeat(len - lo),
                    ]
                }
                .join("")
            }
        }
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Ascii representation of an arc, with a histogram of op locations superimposed.
    /// Each character of the string, if an op falls in that "bucket", will be represented
    /// by a hexadecimal digit representing the number of ops in that bucket,
    /// with a max of 0xF (15)
    pub fn to_ascii_with_ops<L: Into<crate::loc8::Loc8>, I: IntoIterator<Item = L>>(
        &self,
        len: usize,
        ops: I,
    ) -> String {
        use crate::loc8::Loc8;

        let mut buf = vec![0; len];
        let mut s = self.to_ascii(len);
        for o in ops {
            let o: Loc8 = o.into();
            let o: DhtLocation = o.into();
            let loc = loc_downscale(len, o);
            buf[loc] += 1;
        }
        for (i, v) in buf.into_iter().enumerate() {
            if v > 0 {
                // add hex representation of number of ops in this bucket
                let c = format!("{:x}", v.min(0xf));
                s.replace_range(i..i + 1, &c);
            }
        }
        s
    }

    pub fn canonical(self) -> DhtArcRange {
        self
    }
}

/// Check whether a bounded interval is equivalent to the Full interval
pub fn is_full(start: u32, end: u32) -> bool {
    (start == super::dht_arc_set::MIN && end >= super::dht_arc_set::MAX)
        || end == start.wrapping_sub(1)
}

pub fn full_to_half_len(full_len: u64) -> u32 {
    if full_len == 0 {
        0
    } else {
        ((full_len / 2) as u32).wrapping_add(1).min(MAX_HALF_LENGTH)
    }
}

pub fn half_to_full_len(half_len: u32) -> u64 {
    if half_len == 0 {
        0
    } else if half_len == MAX_HALF_LENGTH {
        U32_LEN
    } else {
        (half_len as u64 * 2).wrapping_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_contains() {
        let convergent = DhtArcRange::Bounded(10, 20);
        let divergent = DhtArcRange::Bounded(20, 10);

        assert!(!convergent.contains(0));
        assert!(!convergent.contains(5));
        assert!(convergent.contains(10));
        assert!(convergent.contains(15));
        assert!(convergent.contains(20));
        assert!(!convergent.contains(25));
        assert!(!convergent.contains(u32::MAX));

        assert!(divergent.contains(0));
        assert!(divergent.contains(5));
        assert!(divergent.contains(10));
        assert!(!divergent.contains(15));
        assert!(divergent.contains(20));
        assert!(divergent.contains(25));
        assert!(divergent.contains(u32::MAX));
    }

    #[test]
    fn test_length() {
        let full = 2u64.pow(32);
        assert_eq!(DhtArcRange::Empty.length(), 0);
        assert_eq!(DhtArcRange::from_bounds(0, 0).length(), 1);
        assert_eq!(DhtArcRange::from_bounds(0, 1).length(), 2);
        assert_eq!(DhtArcRange::from_bounds(1, 0).length(), full);
        assert_eq!(DhtArcRange::from_bounds(2, 0).length(), full - 1);
    }

    #[test]
    fn test_ascii() {
        let cent = u32::MAX / 100 + 1;
        assert_eq!(
            DhtArc::from_bounds(cent * 30, cent * 60).to_ascii(10),
            "   @---   ".to_string()
        );
        assert_eq!(
            DhtArc::from_bounds(cent * 33, cent * 63).to_ascii(10),
            "   @---   ".to_string()
        );
        assert_eq!(
            DhtArc::from_bounds(cent * 29, cent * 59).to_ascii(10),
            "  @---    ".to_string()
        );

        assert_eq!(
            DhtArc::from_bounds(cent * 60, cent * 30).to_ascii(10),
            "----  @---".to_string()
        );
        assert_eq!(
            DhtArc::from_bounds(cent * 63, cent * 33).to_ascii(10),
            "----  @---".to_string()
        );
        assert_eq!(
            DhtArc::from_bounds(cent * 59, cent * 29).to_ascii(10),
            "---  @----".to_string()
        );

        assert_eq!(
            DhtArc::from_bounds(cent * 99, cent * 0).to_ascii(10),
            "-        @".to_string()
        );
    }
}
