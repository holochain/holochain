use gcollections::ops::*;
use interval::{interval_set::*, IntervalSet};
use std::{borrow::Borrow, collections::VecDeque, fmt::Debug};

use crate::DhtLocation;

type T = u32;

// For u32, IntervalSet excludes MAX from its set of valid values due to its
// need to be able to express the width of an interval using a u32.
// This min and max are set accordingly.
const MIN: T = T::MIN;
const MAX: T = T::MAX - 1;

#[derive(Clone, PartialEq, Eq)]
pub enum DhtArcSet {
    /// Full coverage.
    /// This needs a special representation because the underlying IntervalSet
    /// implementation excludes `u32::MAX` from its set of valid bounds
    Full,
    /// Any coverage other than full, including empty
    Partial(IntervalSet<T>),
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
            intervals.iter().any(|i| is_full(i.lower(), i.upper()))
        } else {
            false
        };

        if make_full {
            Self::Full
        } else {
            self
        }
    }

    pub fn from_bounds(start: u32, end: u32) -> Self {
        if is_full(start, end) {
            Self::new_full()
        } else {
            Self::Partial(
                if start <= end {
                    vec![(start, end)]
                } else {
                    vec![(MIN, end), (start, MAX)]
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
                let mut intervals: VecDeque<(T, T)> =
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
                        if first.0 == MIN && last.1 >= MAX {
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

    pub fn contains(&self, t: T) -> bool {
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

impl From<Vec<(T, T)>> for DhtArcSet {
    fn from(pairs: Vec<(T, T)>) -> Self {
        pairs
            .into_iter()
            .map(|(a, b)| Self::from(&ArcInterval::new(a, b)))
            .fold(Self::new_empty(), |a, b| a.union(&b))
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
pub enum ArcInterval {
    Empty,
    Full,
    Bounded(T, T),
}

impl ArcInterval {
    pub fn new<V>(start: V, end: V) -> Self
    where
        DhtLocation: From<V>,
    {
        let start = DhtLocation::from(start).to_u32();
        let end = DhtLocation::from(end).to_u32();
        if is_full(start, end) {
            Self::Full
        } else {
            Self::Bounded(start, end)
        }
    }

    /// Constructor
    pub fn new_empty() -> Self {
        Self::Empty
    }

    pub fn from_bounds(bounds: (T, T)) -> Self {
        Self::Bounded(bounds.0, bounds.1)
    }

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

    /// Shift the bounds so that an integer half-length is achieved. Always
    /// increase the half-length, so that the resulting quantized interval is
    /// a superset of the original
    pub fn quantized(&self) -> Self {
        if let Self::Bounded(lo, hi) = self {
            if lo < hi && (hi - lo) % 2 == 1 {
                Self::Bounded(lo.wrapping_sub(1), *hi)
            } else if lo > hi && (lo - hi) % 2 == 1 {
                Self::Bounded(*lo, hi.wrapping_add(1))
            } else {
                self.clone()
            }
        } else {
            self.clone()
        }
    }

    /// Represent an arc as an optional range of inclusive endpoints.
    /// If none, the arc length is 0
    pub fn to_bounds_grouped(&self) -> Option<(u32, u32)> {
        match self {
            Self::Empty => None,
            Self::Full => Some((0, u32::MAX)),
            &Self::Bounded(lo, hi) => Some((lo, hi)),
        }
    }

    /// Same as `to_bounds_grouped`, but with the return type "inside-out"
    pub fn to_bounds_detached(&self) -> (Option<u32>, Option<u32>) {
        self.to_bounds_grouped()
            .map(|(a, b)| (Some(a), Some(b)))
            .unwrap_or_default()
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn to_ascii(&self, len: usize) -> String {
        match self {
            Self::Full => "(FULL)".to_string(),
            Self::Empty => "(EMPTY)".to_string(),
            Self::Bounded(lo, hi) => {
                let factor = len as f64 / u32::MAX as f64;
                let lo = (factor * *lo as f64) as usize;
                let hi = (factor * *hi as f64) as usize;
                if lo <= hi {
                    vec![
                        " ".repeat(lo),
                        "-".repeat(hi - lo + 1),
                        " ".repeat(usize::max(len - hi - 1, 0)),
                    ]
                } else {
                    vec![
                        "-".repeat(hi + 1),
                        " ".repeat(usize::max(lo - hi - 1, 0)),
                        "-".repeat(len - lo),
                    ]
                }
                .join("")
            }
        }
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
            ArcInterval::Bounded(cent * 30, cent * 60).to_ascii(10),
            "   ----   ".to_string()
        );
        assert_eq!(
            ArcInterval::Bounded(cent * 33, cent * 63).to_ascii(10),
            "   ----   ".to_string()
        );
        assert_eq!(
            ArcInterval::Bounded(cent * 29, cent * 59).to_ascii(10),
            "  ----    ".to_string()
        );

        assert_eq!(
            ArcInterval::Bounded(cent * 60, cent * 30).to_ascii(10),
            "----  ----".to_string()
        );
        assert_eq!(
            ArcInterval::Bounded(cent * 63, cent * 33).to_ascii(10),
            "----  ----".to_string()
        );
        assert_eq!(
            ArcInterval::Bounded(cent * 59, cent * 29).to_ascii(10),
            "---  -----".to_string()
        );

        assert_eq!(
            ArcInterval::Bounded(cent * 99, cent * 0).to_ascii(10),
            "-        -".to_string()
        );
    }
}
