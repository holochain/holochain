use gcollections::ops::*;
use interval::{interval_set::*, IntervalSet};
use std::{collections::VecDeque, fmt::Debug};

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

    pub fn from_interval(arc: ArcInterval) -> Self {
        match arc {
            ArcInterval::Full => Self::new_full(),
            ArcInterval::Empty => Self::new_empty(),
            ArcInterval::Bounded(start, end) => Self::from_bounds(start, end),
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

impl From<ArcInterval> for DhtArcSet {
    fn from(arc: ArcInterval) -> Self {
        Self::from_interval(arc)
    }
}

impl From<Vec<ArcInterval>> for DhtArcSet {
    fn from(arcs: Vec<ArcInterval>) -> Self {
        arcs.into_iter()
            .map(Self::from)
            .fold(Self::new_empty(), |a, b| a.union(&b))
    }
}

impl From<Vec<(T, T)>> for DhtArcSet {
    fn from(pairs: Vec<(T, T)>) -> Self {
        pairs
            .into_iter()
            .map(|(a, b)| Self::from(ArcInterval::new(a, b)))
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArcInterval {
    Empty,
    Full,
    Bounded(T, T),
}

impl ArcInterval {
    pub fn new(start: T, end: T) -> Self {
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
}

/// Check whether a bounded interval is equivalent to the Full interval
fn is_full(start: u32, end: u32) -> bool {
    (start == MIN && end >= MAX) || end == start.wrapping_sub(1)
}
