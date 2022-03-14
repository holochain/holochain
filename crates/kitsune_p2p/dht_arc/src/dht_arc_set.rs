use gcollections::ops::*;
use interval::{interval_set::*, IntervalSet};
use std::{borrow::Borrow, collections::VecDeque};

use crate::{is_full, DhtArc, DhtLocation};

// For u32, IntervalSet excludes MAX from its set of valid values due to its
// need to be able to express the width of an interval using a u32.
// This min and max are set accordingly.
pub(crate) const MIN: u32 = u32::MIN;
pub(crate) const MAX: u32 = u32::MAX - 1;

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

    pub fn from_interval<A: Borrow<DhtArc>>(arc: A) -> Self {
        match arc.borrow() {
            DhtArc::Full(_) => Self::new_full(),
            DhtArc::Empty(_) => Self::new_empty(),
            DhtArc::Bounded(start, end) => Self::from_bounds(*start, *end),
        }
    }

    pub fn intervals(&self) -> Vec<DhtArc> {
        match self {
            // XXX: loss of information here, this DhtArc
            //      does not actually know about its true start
            Self::Full => vec![DhtArc::Full(0u32.into())],
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
                    .map(|(lo, hi)| DhtArc::from_bounds(lo, hi))
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

impl From<&DhtArc> for DhtArcSet {
    fn from(arc: &DhtArc) -> Self {
        Self::from_interval(arc)
    }
}

impl From<DhtArc> for DhtArcSet {
    fn from(arc: DhtArc) -> Self {
        Self::from_interval(arc)
    }
}

impl From<&[DhtArc]> for DhtArcSet {
    fn from(arcs: &[DhtArc]) -> Self {
        arcs.iter()
            .map(Self::from)
            .fold(Self::new_empty(), |a, b| a.union(&b))
    }
}

impl From<Vec<DhtArc>> for DhtArcSet {
    fn from(arcs: Vec<DhtArc>) -> Self {
        arcs.iter()
            .map(Self::from)
            .fold(Self::new_empty(), |a, b| a.union(&b))
    }
}

impl From<Vec<(DhtLocation, DhtLocation)>> for DhtArcSet {
    fn from(pairs: Vec<(DhtLocation, DhtLocation)>) -> Self {
        pairs
            .into_iter()
            .map(|(a, b)| Self::from(&DhtArc::from_bounds(a, b)))
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
