use crate::op::{Loc, Timestamp};

pub type Coord = u32;

pub struct SpacetimeCoords {
    pub space: Coord,
    pub time: Coord,
}

impl SpacetimeCoords {
    pub fn to_tuple(&self) -> (u32, u32) {
        (self.space, self.time)
    }
}

/// Any interval in space or time is represented by a node in a tree, so our
/// way of describing intervals uses tree coordinates as well:
/// The length of an interval is 2^(height), and the position of its left edge
/// is at (offset * length).
#[derive(Copy, Clone, Debug)]
pub struct Interval {
    pub height: u32,
    pub offset: u32,
}

impl Interval {
    pub fn length(&self) -> u64 {
        // If height is 32, this overflows a u32
        2u64.pow(self.height)
    }

    pub fn bounds(&self) -> (Coord, Coord) {
        let l = self.length();
        let o = self.offset as u64;
        ((o * l) as u32, (o * l + l - 1) as u32)
    }

    /// Halving an interval is equivalent to taking the child nodes of the node
    /// which represents this interval
    pub fn halve(self) -> Option<(Self, Self)> {
        if self.height == 0 {
            // Can't split a quantum value (a leaf has no children)
            None
        } else {
            let height = self.height - 1;
            Some((
                Interval {
                    height,
                    offset: self.offset * 2,
                },
                Interval {
                    height,
                    offset: self.offset * 2 + 1,
                },
            ))
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct RegionCoords {
    pub space: Interval,
    pub time: Interval,
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
}

#[derive(Clone, Debug)]
pub struct Dimension {
    /// The smallest possible length in this dimension.
    /// Determines the interval represented by the leaf of a tree.
    quantum: u32,
    /// The largest possible value; the size of this dimension.
    size: u32,
    /// The number of bits used to represent a coordinate
    bit_depth: u8,
}

/// Parameters which are constant for all time trees in a given network.
/// They determine the relationship between tree structure and absolute time.
#[derive(Clone, Debug)]
pub struct Topology {
    pub space: Dimension,
    pub time: Dimension,
}

impl Topology {
    const MAX_SPACE: u32 = u32::MAX;

    pub fn standard() -> Self {
        Self {
            space: Dimension {
                quantum: 1,
                size: u32::MAX,
                bit_depth: 32,
            },
            time: Dimension {
                quantum: 1,
                size: u32::MAX,
                bit_depth: 32,
            },
        }
    }

    pub fn space_coord(&self, loc: Loc) -> Coord {
        todo!()
    }

    pub fn time_coord(&self, timestamp: Timestamp) -> Coord {
        todo!()
    }
}
