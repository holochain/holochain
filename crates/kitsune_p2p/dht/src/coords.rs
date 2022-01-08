use std::{marker::PhantomData, ops::Deref};

use crate::op::{Loc, Timestamp};

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, derive_more::From, derive_more::Deref,
)]
pub struct SpaceCoord(u32);

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, derive_more::From, derive_more::Deref,
)]
pub struct TimeCoord(u32);

pub trait Coord: From<u32> + Deref<Target = u32> {
    const MAX: u32 = u32::MAX;
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
#[derive(Copy, Clone, Debug)]
pub struct Interval<C: Coord> {
    pub power: u32,
    pub offset: u32,
    phantom: PhantomData<C>,
}

impl<C: Coord> Interval<C> {
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
                Interval {
                    power,
                    offset: self.offset * 2,
                    phantom: PhantomData,
                },
                Interval {
                    power,
                    offset: self.offset * 2 + 1,
                    phantom: PhantomData,
                },
            ))
        }
    }
}

pub type SpaceInterval = Interval<SpaceCoord>;
pub type TimeInterval = Interval<TimeCoord>;

#[derive(Copy, Clone, Debug)]
pub struct RegionCoords {
    pub space: SpaceInterval,
    pub time: TimeInterval,
}

impl RegionCoords {
    pub fn halve(self) -> Option<(Self, Self)> {
        let (sa, sb) = self.space.halve()?;
        Some((
            Self {
                space: sa,
                time: self.time,
            },
            Self {
                space: sb,
                time: self.time,
            },
        ))
    }

    pub fn to_bounds(&self) -> RegionBounds {
        RegionBounds {
            x: self.space.bounds(),
            t: self.time.bounds(),
        }
    }
}

pub struct RegionBounds {
    pub x: (SpaceCoord, SpaceCoord),
    pub t: (TimeCoord, TimeCoord),
}

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
    pub fn identity() -> Self {
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

    pub fn identity(time_origin: Timestamp) -> Self {
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
}
