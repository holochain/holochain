use gcollections::ops::*;
use interval::{interval_set::*, IntervalSet};
use std::{collections::VecDeque, fmt::Debug};

type T = u32;
const MIN: T = T::MIN;
const MAX: T = T::MAX;

#[derive(Clone, PartialEq, Eq)]
pub struct DhtArcSet(IntervalSet<T>);

impl std::fmt::Debug for DhtArcSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "DhtArcSet({:#?})",
            self.0.iter().collect::<Vec<_>>()
        ))
    }
}

impl DhtArcSet {
    pub fn new_empty() -> Self {
        Self(vec![].to_interval_set())
    }

    pub fn from_interval(wint: ArcInterval) -> Self {
        match wint {
            ArcInterval::Full => Self(vec![(MIN, MAX)].to_interval_set()),
            ArcInterval::Empty => Self(vec![].to_interval_set()),
            ArcInterval::Bounded(start, end) => Self(
                if start <= end {
                    vec![(start, end)]
                } else {
                    vec![(MIN, end), (start, MAX)]
                }
                .to_interval_set(),
            ),
        }
    }

    pub fn intervals(&self) -> Vec<ArcInterval> {
        let intervals = &self.0;
        let mut intervals: VecDeque<(T, T)> =
            intervals.iter().map(|i| (i.lower(), i.upper())).collect();
        let wrapping = match (intervals.front(), intervals.back()) {
            (Some(first), Some(last)) => {
                if first.0 == MIN && *&last.1 == MAX {
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

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Cheap check if the two sets have a non-null intersection
    pub fn overlap(&self, other: &Self) -> bool {
        dbg!(dbg!(self).intersection(dbg!(other)));
        self.0.overlap(&other.0)
    }

    pub fn union(&self, other: &Self) -> Self {
        Self(self.0.union(&other.0))
    }

    pub fn intersection(&self, other: &Self) -> Self {
        Self(self.0.intersection(&other.0))
    }
}

impl From<ArcInterval> for DhtArcSet {
    fn from(wint: ArcInterval) -> Self {
        Self::from_interval(wint)
    }
}

impl From<Vec<ArcInterval>> for DhtArcSet {
    fn from(wints: Vec<ArcInterval>) -> Self {
        wints
            .into_iter()
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

/// An alternate implementation of `ArcRange`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArcInterval {
    Empty,
    Full,
    Bounded(T, T),
}

impl ArcInterval {
    pub fn new(start: T, end: T) -> Self {
        Self::Bounded(start, end)
    }

    /// Constructor
    pub fn new_empty() -> Self {
        Self::Empty
    }

    pub fn from_bounds(bounds: (T, T)) -> Self {
        Self::Bounded(bounds.0, bounds.1)
    }
}
