//! A type for indicating ranges on the dht arc

use std::ops::Bound;
use std::ops::RangeBounds;

use crate::*;

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
        unimplemented!("Contains doesn't make sense for this type of range due to redundant holding near the bounds. Use DhtArc::contains")
    }
}

/// An alternate implementation of `ArcRange`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DhtArc<T = DhtLocation> {
    Empty(T),
    Full(T),
    Bounded(T, T),
}

impl<T: PartialOrd + num_traits::Num> DhtArc<T> {
    pub fn contains<B: std::borrow::Borrow<T>>(&self, t: B) -> bool {
        match self {
            Self::Empty(_) => false,
            Self::Full(_) => true,
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

impl<T> DhtArc<T> {
    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> DhtArc<U> {
        match self {
            Self::Empty(s) => DhtArc::Empty(f(s)),
            Self::Full(s) => DhtArc::Full(f(s)),
            Self::Bounded(lo, hi) => DhtArc::Bounded(f(lo), f(hi)),
        }
    }

    #[deprecated = "left over from refactor"]
    pub fn interval(self) -> Self {
        self
    }
}

impl<T: num_traits::AsPrimitive<u32>> DhtArc<T> {
    pub fn from_bounds(start: T, end: T) -> DhtArc<DhtLocation> {
        let start = start.as_();
        let end = end.as_();
        if is_full(start, end) {
            DhtArc::Full(start.into())
        } else {
            DhtArc::Bounded(DhtLocation::new(start), DhtLocation::new(end))
        }
    }

    pub fn from_start_and_len(start: T, len: u64) -> DhtArc<DhtLocation> {
        let start = start.as_();
        if len == 0 {
            DhtArc::Empty(start.into())
        } else {
            let end = start.wrapping_add((len - 1) as u32);
            DhtArc::from_bounds(start, end)
        }
    }

    /// Convenience for our legacy code which defined arcs in terms of half-lengths
    /// rather than full lengths
    pub fn from_start_and_half_len(start: T, halflen: u32) -> DhtArc<DhtLocation> {
        Self::from_start_and_len(start, (halflen as u64) * 2 - 1)
    }

    pub fn new_generic(start: T, end: T) -> Self {
        if is_full(start.as_(), end.as_()) {
            Self::Full(start)
        } else {
            Self::Bounded(start, end)
        }
    }
}

impl DhtArc<u32> {
    pub fn canonical(self) -> DhtArc {
        match self {
            DhtArc::Empty(s) => DhtArc::Empty(DhtLocation::new(s)),
            DhtArc::Full(s) => DhtArc::Full(DhtLocation::new(s)),
            DhtArc::Bounded(lo, hi) => {
                DhtArc::from_bounds(DhtLocation::new(lo), DhtLocation::new(hi))
            }
        }
    }
}

impl DhtArc<DhtLocation> {
    /// Constructor
    pub fn new_empty(s: DhtLocation) -> Self {
        Self::Empty(s)
    }

    /// Represent an arc as an optional range of inclusive endpoints.
    /// If none, the arc length is 0
    pub fn to_bounds_grouped(&self) -> Option<(DhtLocation, DhtLocation)> {
        match self {
            Self::Empty(_) => None,
            Self::Full(s) => Some((*s, s.as_u32().wrapping_sub(1).into())),
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
        matches!(self, Self::Empty(_))
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

    pub fn start_loc(&self) -> DhtLocation {
        match self {
            DhtArc::Empty(s) => *s,
            DhtArc::Full(s) => *s,
            DhtArc::Bounded(s, _) => *s,
        }
    }

    /// Get the range of the arc
    pub fn range(&self) -> ArcRange {
        match self {
            DhtArc::Empty(s) => ArcRange {
                start: Bound::Excluded(s.as_u32()),
                end: Bound::Excluded(s.as_u32()),
            },
            DhtArc::Full(s) => ArcRange {
                start: Bound::Included(s.as_u32()),
                end: Bound::Included(s.as_u32().wrapping_sub(1)),
            },
            DhtArc::Bounded(lo, hi) => ArcRange {
                start: Bound::Included(lo.as_u32()),
                end: Bound::Included(hi.as_u32()),
            },
        }
    }

    /// The percentage of the full circle that is covered
    /// by this arc.
    pub fn coverage(&self) -> f64 {
        self.length() as f64 / 2f64.powf(32.0)
    }

    pub fn length(&self) -> u64 {
        match self {
            DhtArc::Empty(_) => 0,
            DhtArc::Full(_) => 2u64.pow(32),
            DhtArc::Bounded(lo, hi) => hi.as_u32().wrapping_sub(lo.as_u32()).into(),
        }
    }

    // #[deprecated = "leftover from refactor"]
    pub fn half_length(&self) -> u32 {
        (self.length() / 2) as u32 + 1
    }

    /// Update the half length based on a PeerView reading.
    /// This will converge on a new target instead of jumping directly
    /// to the new target and is designed to be called at a given rate
    /// with more recent peer views.
    pub fn update_length<V: Into<PeerView>>(&mut self, view: V) {
        let new_length = (U32_LEN as f64 * view.into().next_coverage(self.coverage())) as u64;
        *self = Self::from_start_and_len(self.start_loc(), new_length)
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
            Self::Full(_) => full(),
            Self::Empty(_) => empty(),
            Self::Bounded(lo0, hi0) => {
                let lo = loc_downscale(len, *lo0);
                let hi = loc_downscale(len, *hi0);
                let mut s = if lo0 <= hi0 {
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
                .join("");
                let start = loc_downscale(len, self.start_loc());
                s.replace_range(start..start + 1, "@");
                s
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

    pub fn canonical(self) -> DhtArc {
        self
    }
}

/// Check whether a bounded interval is equivalent to the Full interval
pub fn is_full(start: u32, end: u32) -> bool {
    (start == super::dht_arc_set::MIN && end >= super::dht_arc_set::MAX)
        || end == start.wrapping_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_contains() {
        let convergent = DhtArc::Bounded(10, 20);
        let divergent = DhtArc::Bounded(20, 10);

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
    fn test_ascii() {
        let cent = u32::MAX / 100 + 1;
        assert_eq!(
            DhtArc::from_bounds(cent * 30, cent * 60).to_ascii(10),
            "   -@--   ".to_string()
        );
        assert_eq!(
            DhtArc::from_bounds(cent * 33, cent * 63).to_ascii(10),
            "   -@--   ".to_string()
        );
        assert_eq!(
            DhtArc::from_bounds(cent * 29, cent * 59).to_ascii(10),
            "  --@-    ".to_string()
        );

        assert_eq!(
            DhtArc::from_bounds(cent * 60, cent * 30).to_ascii(10),
            "----  ---@".to_string()
        );
        assert_eq!(
            DhtArc::from_bounds(cent * 63, cent * 33).to_ascii(10),
            "----  ---@".to_string()
        );
        assert_eq!(
            DhtArc::from_bounds(cent * 59, cent * 29).to_ascii(10),
            "---  ----@".to_string()
        );

        assert_eq!(
            DhtArc::from_bounds(cent * 99, cent * 0).to_ascii(10),
            "-        @".to_string()
        );
    }
}
