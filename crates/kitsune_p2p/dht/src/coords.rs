use std::{
    marker::PhantomData,
    ops::{Deref, ShrAssign},
};

use num_traits::Zero;

use crate::op::{Loc, Timestamp};

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    derive_more::Add,
    derive_more::Deref,
    derive_more::Display,
    derive_more::From,
)]
pub struct SpaceCoord(u32);

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    derive_more::Add,
    derive_more::Deref,
    derive_more::Display,
    derive_more::From,
)]
pub struct TimeCoord(u32);

impl TimeCoord {
    pub fn from_timestamp(topo: &Topology, timestamp: Timestamp) -> Self {
        topo.time_coord(timestamp)
    }
}

pub trait Coord: From<u32> + Deref<Target = u32> {
    const MAX: u32 = u32::MAX;

    fn exp(&self, pow: u8) -> u32 {
        **self * 2u32.pow(pow as u32)
    }

    fn exp_wrapping(&self, pow: u8) -> u32 {
        (**self as u64 * 2u64.pow(pow as u32)) as u32
    }

    fn wrapping_add(self, other: u32) -> Self {
        Self::from((*self).wrapping_add(other))
    }

    fn wrapping_sub(self, other: u32) -> Self {
        Self::from((*self).wrapping_sub(other))
    }
}

impl Coord for SpaceCoord {}
impl Coord for TimeCoord {}

pub struct SpacetimeCoords {
    pub space: SpaceCoord,
    pub time: TimeCoord,
}

impl SpacetimeCoords {
    pub fn to_tuple(&self) -> (u32, u32) {
        (self.space.0, self.time.0)
    }
}

/// Any interval in space or time is represented by a node in a tree, so our
/// way of describing intervals uses tree coordinates as well:
/// The length of an interval is 2^(power), and the position of its left edge
/// is at (offset * length).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Segment<C: Coord> {
    // TODO: make `u8`?
    pub power: u32,
    pub offset: u32,
    phantom: PhantomData<C>,
}

impl<C: Coord> Segment<C> {
    pub fn new(power: u32, offset: u32) -> Self {
        Self {
            power,
            offset,
            phantom: PhantomData,
        }
    }

    pub fn length(&self) -> u64 {
        // If power is 32, this overflows a u32
        2u64.pow(self.power)
    }

    pub fn bounds(&self) -> (C, C) {
        let l = self.length();
        let o = self.offset as u64;
        (C::from((o * l) as u32), C::from((o * l + l - 1) as u32))
    }

    /// Halving an interval is equivalent to taking the child nodes of the node
    /// which represents this interval
    pub fn halve(self) -> Option<(Self, Self)> {
        if self.power == 0 {
            // Can't split a quantum value (a leaf has no children)
            None
        } else {
            let power = self.power - 1;
            Some((
                Segment::new(power, self.offset * 2),
                Segment::new(power, self.offset * 2 + 1),
            ))
        }
    }
}

const D: Dimension = Dimension {
    quantum: 1,
    size: 1,
    bit_depth: 1,
};

pub type SpaceSegment = Segment<SpaceCoord>;
pub type TimeSegment = Segment<TimeCoord>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dimension {
    /// The smallest possible length in this dimension.
    /// Determines the interval represented by the leaf of a tree.
    quantum: u32,
    /// The largest possible value; the size of this dimension.
    size: u32,
    /// The number of bits used to represent a coordinate
    bit_depth: u8,
}

impl Dimension {
    pub const fn identity() -> Self {
        Dimension {
            quantum: 1,
            size: u32::MAX,
            bit_depth: 32,
        }
    }
}

/// Parameters which are constant for all time trees in a given network.
/// They determine the relationship between tree structure and absolute time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topology {
    pub space: Dimension,
    pub time: Dimension,
    pub time_origin: Timestamp,
}

impl Topology {
    const MAX_SPACE: u32 = u32::MAX;

    pub const fn identity(time_origin: Timestamp) -> Self {
        Self {
            space: Dimension::identity(),
            time: Dimension::identity(),
            time_origin,
        }
    }

    pub fn space_coord(&self, loc: Loc) -> SpaceCoord {
        assert_eq!(
            self.space,
            Dimension::identity(),
            "Alternate quantizations of space are not yet supported"
        );
        (loc.as_u32()).into()
    }

    pub fn time_coord(&self, timestamp: Timestamp) -> TimeCoord {
        assert_eq!(
            self.time,
            Dimension::identity(),
            "Alternate quantizations of time are not yet supported"
        );
        (timestamp.as_micros() as u32).into()
    }

    /// Calculate the list of exponentially shrinking time windows, as per
    /// this document: https://hackmd.io/@hololtd/r1IAIbr5Y
    pub fn telescoping_times(&self, now: Timestamp) -> Vec<TimeSegment> {
        let mut now: u32 = *self.time_coord(now) + 1;
        if now == 1 {
            return vec![];
        }
        let zs = now.leading_zeros();
        now <<= zs;
        let mut seg = TimeSegment::new(32 - zs - 1, 0);
        let mut times = vec![];
        let mask = 1u32.rotate_right(1); // 0b100000...
        for _ in 0..(32 - zs - 1) {
            seg.power -= 1;
            now &= !mask;
            now <<= 1;

            times.push(seg);
            if now & mask > 0 {
                times.push(seg);
            }
            seg.offset += 2u32.pow(seg.power + 1);
        }
        times
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_length() {
        let s = TimeSegment {
            power: 31,
            offset: 0,
            phantom: PhantomData,
        };
        assert_eq!(s.length(), 2u64.pow(31));
    }

    fn lengths(topo: &Topology, t: u32) -> Vec<u32> {
        topo.telescoping_times(Timestamp::from_micros(t as i64))
            .into_iter()
            .map(|i| i.length() as u32)
            .collect()
    }

    #[test]
    fn test_telescoping_times_first_16_identity_topology() {
        let topo = Topology::identity(Timestamp::from_micros(0));

        assert_eq!(lengths(&topo, 0), Vec::<u32>::new());
        assert_eq!(lengths(&topo, 1), vec![1]);
        assert_eq!(lengths(&topo, 2), vec![1, 1]);
        assert_eq!(lengths(&topo, 3), vec![2, 1]);
        assert_eq!(lengths(&topo, 4), vec![2, 1, 1]);
        assert_eq!(lengths(&topo, 5), vec![2, 2, 1]);
        assert_eq!(lengths(&topo, 6), vec![2, 2, 1, 1]);
        assert_eq!(lengths(&topo, 7), vec![4, 2, 1]);
        assert_eq!(lengths(&topo, 8), vec![4, 2, 1, 1]);
        assert_eq!(lengths(&topo, 9), vec![4, 2, 2, 1]);
        assert_eq!(lengths(&topo, 10), vec![4, 2, 2, 1, 1]);
        assert_eq!(lengths(&topo, 11), vec![4, 4, 2, 1]);
        assert_eq!(lengths(&topo, 12), vec![4, 4, 2, 1, 1]);
        assert_eq!(lengths(&topo, 13), vec![4, 4, 2, 2, 1]);
        assert_eq!(lengths(&topo, 14), vec![4, 4, 2, 2, 1, 1]);
        assert_eq!(lengths(&topo, 15), vec![8, 4, 2, 1]);
    }

    #[test]
    fn test_telescoping_times_first_16_standard_topology() {
        let topo = todo!("other time topology");

        assert_eq!(lengths(&topo, 0), Vec::<u32>::new());
        assert_eq!(lengths(&topo, 1), vec![1]);
        assert_eq!(lengths(&topo, 2), vec![1, 1]);
        assert_eq!(lengths(&topo, 3), vec![2, 1]);
        assert_eq!(lengths(&topo, 4), vec![2, 1, 1]);
        assert_eq!(lengths(&topo, 5), vec![2, 2, 1]);
        assert_eq!(lengths(&topo, 6), vec![2, 2, 1, 1]);
        assert_eq!(lengths(&topo, 7), vec![4, 2, 1]);
        assert_eq!(lengths(&topo, 8), vec![4, 2, 1, 1]);
        assert_eq!(lengths(&topo, 9), vec![4, 2, 2, 1]);
        assert_eq!(lengths(&topo, 10), vec![4, 2, 2, 1, 1]);
        assert_eq!(lengths(&topo, 11), vec![4, 4, 2, 1]);
        assert_eq!(lengths(&topo, 12), vec![4, 4, 2, 1, 1]);
        assert_eq!(lengths(&topo, 13), vec![4, 4, 2, 2, 1]);
        assert_eq!(lengths(&topo, 14), vec![4, 4, 2, 2, 1, 1]);
        assert_eq!(lengths(&topo, 15), vec![8, 4, 2, 1]);
    }

    proptest::proptest! {
        #[test]
        fn telescoping_times_fit_total_time_span(now in 0i64..u32::MAX as i64) {
            let topo = Topology::identity(Timestamp::from_micros(0));
            let ts = topo.telescoping_times(Timestamp::from_micros(now));
            assert_eq!(ts.iter().map(TimeSegment::length).sum::<u64>(), now as u64);
        }

        #[test]
        fn telescoping_times_end_with_1(now: i64) {
            let topo = Topology::identity(Timestamp::from_micros(0));
            if let Some(last) = topo.telescoping_times(Timestamp::from_micros(now)).pop() {
                assert_eq!(last.power, 0);
            }
        }
    }
}
