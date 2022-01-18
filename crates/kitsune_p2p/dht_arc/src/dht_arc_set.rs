use gcollections::ops::*;
use interval::{interval_set::*, IntervalSet};
use std::{borrow::Borrow, collections::VecDeque, fmt::Debug};

use crate::{DhtArc, DhtLocation};

// For u32, IntervalSet excludes MAX from its set of valid values due to its
// need to be able to express the width of an interval using a u32.
// This min and max are set accordingly.
const MIN: u32 = u32::MIN;
const MAX: u32 = u32::MAX - 1;

#[derive(Clone, PartialEq, Eq)]
pub enum DhtArcSet {
    /// Full coverage.
    /// This needs a special representation because the underlying IntervalSet
    /// implementation excludes `u32::MAX` from its set of valid bounds
    Full,
    /// Any coverage other than full, including empty
    Partial(IntervalSet<DhtLocation>),
}

impl std::fmt::Debug for DhtArcSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => f.write_fmt(format_args!("DhtArcSet(Full)",)),
            Self::Partial(intervals) => f.write_fmt(format_args!(
                "DhtArcSet({:#?})",
                intervals.iter().collect::<Vec<_>>()
            )),
        }
    }
}

impl DhtArcSet {
    pub fn new_empty() -> Self {
        Self::Partial(vec![].to_interval_set())
    }

    pub fn new_full() -> Self {
        Self::Full
    }

    pub fn normalized(self) -> Self {
        let make_full = if let Self::Partial(intervals) = &self {
            intervals
                .iter()
                .any(|i| is_full(i.lower().into(), i.upper().into()))
        } else {
            false
        };

        if make_full {
            Self::Full
        } else {
            self
        }
    }

    pub fn from_bounds(start: DhtLocation, end: DhtLocation) -> Self {
        if is_full(start.into(), end.into()) {
            Self::new_full()
        } else {
            let start = start.as_u32().min(MAX).into();
            let end = end.as_u32().min(MAX).into();
            Self::Partial(
                if start <= end {
                    vec![(start, end)]
                } else {
                    vec![(MIN.into(), end), (start, MAX.into())]
                }
                .to_interval_set(),
            )
        }
    }

    pub fn from_interval<A: Borrow<ArcInterval>>(arc: A) -> Self {
        match arc.borrow() {
            ArcInterval::Full => Self::new_full(),
            ArcInterval::Empty => Self::new_empty(),
            ArcInterval::Bounded(start, end) => Self::from_bounds(*start, *end),
        }
    }

    pub fn intervals(&self) -> Vec<ArcInterval> {
        match self {
            Self::Full => vec![ArcInterval::Full],
            Self::Partial(intervals) => {
                let mut intervals: VecDeque<(DhtLocation, DhtLocation)> =
                    intervals.iter().map(|i| (i.lower(), i.upper())).collect();
                let wrapping = match (intervals.front(), intervals.back()) {
                    (Some(first), Some(last)) => {
                        // if there is an interval at the very beginning and one
                        // at the very end, let's interpret it as a single
                        // wrapping interval.
                        //
                        // NB: this checks for values greater than the MAX,
                        // because MAX is not u32::MAX. We don't expect values
                        // greater than MAX, but it's no harm if we do see one.
                        if first.0.as_u32() == MIN && last.1.as_u32() >= MAX {
                            Some((last.0, first.1))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                // Condense the two bookend intervals into single wrapping interval
                if let Some(wrapping) = wrapping {
                    intervals.pop_front();
                    intervals.pop_back();
                    intervals.push_back(wrapping);
                }
                intervals
                    .into_iter()
                    .map(ArcInterval::from_bounds)
                    .collect()
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Full => false,
            Self::Partial(intervals) => intervals.is_empty(),
        }
    }

    pub fn contains(&self, t: DhtLocation) -> bool {
        self.overlap(&DhtArcSet::from(vec![(t, t)]))
    }

    /// Cheap check if the two sets have a non-null intersection
    pub fn overlap(&self, other: &Self) -> bool {
        match (self, other) {
            (this, Self::Full) => !this.is_empty(),
            (Self::Full, that) => !that.is_empty(),
            (Self::Partial(this), Self::Partial(that)) => this.overlap(that),
        }
    }

    pub fn union(&self, other: &Self) -> Self {
        match (self, other) {
            (_, Self::Full) => Self::Full,
            (Self::Full, _) => Self::Full,
            (Self::Partial(this), Self::Partial(that)) => {
                Self::Partial(this.union(that)).normalized()
            }
        }
    }

    pub fn intersection(&self, other: &Self) -> Self {
        match (self, other) {
            (this, Self::Full) => this.clone(),
            (Self::Full, that) => that.clone(),
            (Self::Partial(this), Self::Partial(that)) => {
                Self::Partial(this.intersection(that)).normalized()
            }
        }
    }

    pub fn size(&self) -> u32 {
        match self {
            Self::Full => u32::MAX,
            Self::Partial(intervals) => intervals.size(),
        }
    }
}

impl From<&ArcInterval> for DhtArcSet {
    fn from(arc: &ArcInterval) -> Self {
        Self::from_interval(arc)
    }
}

impl From<ArcInterval> for DhtArcSet {
    fn from(arc: ArcInterval) -> Self {
        Self::from_interval(arc)
    }
}

impl From<&[ArcInterval]> for DhtArcSet {
    fn from(arcs: &[ArcInterval]) -> Self {
        arcs.iter()
            .map(Self::from)
            .fold(Self::new_empty(), |a, b| a.union(&b))
    }
}

impl From<Vec<ArcInterval>> for DhtArcSet {
    fn from(arcs: Vec<ArcInterval>) -> Self {
        arcs.iter()
            .map(Self::from)
            .fold(Self::new_empty(), |a, b| a.union(&b))
    }
}

impl From<Vec<(DhtLocation, DhtLocation)>> for DhtArcSet {
    fn from(pairs: Vec<(DhtLocation, DhtLocation)>) -> Self {
        pairs
            .into_iter()
            .map(|(a, b)| Self::from(&ArcInterval::new(a, b)))
            .fold(Self::new_empty(), |a, b| a.union(&b))
    }
}

impl From<Vec<(u32, u32)>> for DhtArcSet {
    fn from(pairs: Vec<(u32, u32)>) -> Self {
        Self::from(
            pairs
                .into_iter()
                .map(|(a, b)| (DhtLocation::new(a), DhtLocation::new(b)))
                .collect::<Vec<_>>(),
        )
    }
}

#[test]
fn fullness() {
    assert_eq!(DhtArcSet::from(vec![(0, u32::MAX),]), DhtArcSet::Full,);
    assert_eq!(DhtArcSet::from(vec![(0, u32::MAX - 1),]), DhtArcSet::Full,);
    assert_ne!(DhtArcSet::from(vec![(0, u32::MAX - 2),]), DhtArcSet::Full,);

    assert_eq!(DhtArcSet::from(vec![(11, 10),]), DhtArcSet::Full,);

    assert_eq!(
        DhtArcSet::from(vec![(u32::MAX - 1, u32::MAX - 2),]),
        DhtArcSet::Full,
    );

    assert_eq!(
        DhtArcSet::from(vec![(u32::MAX, u32::MAX - 1),]),
        DhtArcSet::Full,
    );
}

/// An alternate implementation of `ArcRange`
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ArcInterval<T = DhtLocation> {
    Empty,
    Full,
    Bounded(T, T),
}

impl<T: PartialOrd + num_traits::Num> ArcInterval<T> {
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

impl<T> ArcInterval<T> {
    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> ArcInterval<U> {
        match self {
            Self::Empty => ArcInterval::Empty,
            Self::Full => ArcInterval::Full,
            Self::Bounded(lo, hi) => ArcInterval::Bounded(f(lo), f(hi)),
        }
    }
}

impl<T: num_traits::AsPrimitive<u32>> ArcInterval<T> {
    pub fn new(start: T, end: T) -> ArcInterval<DhtLocation> {
        let start = start.as_();
        let end = end.as_();
        if is_full(start, end) {
            ArcInterval::Full
        } else {
            ArcInterval::Bounded(DhtLocation::new(start), DhtLocation::new(end))
        }
    }

    pub fn new_generic(start: T, end: T) -> Self {
        if is_full(start.as_(), end.as_()) {
            Self::Full
        } else {
            Self::Bounded(start, end)
        }
    }

    pub fn length(&self) -> u64 {
        match self {
            ArcInterval::Empty => 0,
            ArcInterval::Full => 2u64.pow(32),
            ArcInterval::Bounded(lo, hi) => {
                let lo = lo.as_();
                let hi = hi.as_();
                if is_full(lo, hi) {
                    2u64.pow(32)
                } else {
                    (hi).wrapping_sub(lo).wrapping_add(1) as u64
                }
            }
        }
    }
}

impl ArcInterval<u32> {
    pub fn canonical(self) -> ArcInterval {
        match self {
            ArcInterval::Empty => ArcInterval::Empty,
            ArcInterval::Full => ArcInterval::Full,
            ArcInterval::Bounded(lo, hi) => {
                ArcInterval::new(DhtLocation::new(lo), DhtLocation::new(hi))
            }
        }
    }
}

impl ArcInterval<DhtLocation> {
    /// Constructor
    pub fn new_empty() -> Self {
        Self::Empty
    }

    pub fn from_bounds(bounds: (DhtLocation, DhtLocation)) -> Self {
        Self::Bounded(bounds.0, bounds.1)
    }

    /// Shift the bounds so that an integer half-length is achieved. Always
    /// increase the half-length, so that the resulting quantized interval is
    /// a superset of the original
    pub fn quantized(&self) -> Self {
        if let Self::Bounded(lo, hi) = self {
            let lo = *lo;
            let hi = *hi;
            let gap = if lo > hi {
                lo - hi
            } else {
                DhtLocation::from(u32::MAX) - hi + lo
            };
            if gap <= 2.into() {
                // Because a halflen must be even, a small gap leads to full coverage
                Self::Full
            } else if (hi - lo) % 2.into() == 1.into() {
                Self::Bounded(lo, hi + 1.into())
            } else {
                self.clone()
            }
        } else {
            self.clone()
        }
    }

    /// Represent an arc as an optional range of inclusive endpoints.
    /// If none, the arc length is 0
    pub fn to_bounds_grouped(&self) -> Option<(DhtLocation, DhtLocation)> {
        match self {
            Self::Empty => None,
            Self::Full => Some((u32::MIN.into(), u32::MAX.into())),
            &Self::Bounded(lo, hi) => Some((lo, hi)),
        }
    }

    /// Same as primitive_range, but with the return type "inside-out"
    pub fn primitive_range_detached(&self) -> (Option<DhtLocation>, Option<DhtLocation>) {
        self.to_bounds_grouped()
            .map(|(a, b)| (Some(a), Some(b)))
            .unwrap_or_default()
    }

    /// Check if this arc is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
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

    pub fn center_loc(&self) -> DhtLocation {
        DhtArc::from_interval(self.clone()).center_loc()
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Handy ascii representation of an arc, especially useful when
    /// looking at several arcs at once to get a sense of their overlap
    pub fn to_ascii(&self, len: usize) -> String {
        use crate::{loc_downscale, loc_upscale};

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
                let center = loc_downscale(len, self.center_loc());
                s.replace_range(center..center + 1, "@");
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
        use crate::{loc8::Loc8, loc_downscale};

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

    pub fn canonical(self) -> ArcInterval {
        self
    }
}

/// Check whether a bounded interval is equivalent to the Full interval
fn is_full(start: u32, end: u32) -> bool {
    (start == MIN && end >= MAX) || end == start.wrapping_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_length() {
        assert_eq!(ArcInterval::Bounded(10, 20).length(), 11);
        assert_eq!(ArcInterval::Bounded(-10, 0).length(), 11);
        assert_eq!(ArcInterval::Bounded(1, 0).length(), 2u64.pow(32));
        assert_eq!(
            ArcInterval::Bounded(0, u32::MAX / 2).length(),
            u32::MAX as u64 / 2 + 1
        );
        assert_eq!(
            ArcInterval::Bounded(u32::MAX / 2, 0).length(),
            u32::MAX as u64 / 2 + 3
        );
    }

    #[test]
    fn arc_contains() {
        let convergent = ArcInterval::Bounded(10, 20);
        let divergent = ArcInterval::Bounded(20, 10);

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
            ArcInterval::new(cent * 30, cent * 60).to_ascii(10),
            "   -@--   ".to_string()
        );
        assert_eq!(
            ArcInterval::new(cent * 33, cent * 63).to_ascii(10),
            "   -@--   ".to_string()
        );
        assert_eq!(
            ArcInterval::new(cent * 29, cent * 59).to_ascii(10),
            "  --@-    ".to_string()
        );

        assert_eq!(
            ArcInterval::new(cent * 60, cent * 30).to_ascii(10),
            "----  ---@".to_string()
        );
        assert_eq!(
            ArcInterval::new(cent * 63, cent * 33).to_ascii(10),
            "----  ---@".to_string()
        );
        assert_eq!(
            ArcInterval::new(cent * 59, cent * 29).to_ascii(10),
            "---  ----@".to_string()
        );

        assert_eq!(
            ArcInterval::new(cent * 99, cent * 0).to_ascii(10),
            "-        @".to_string()
        );
    }
}
