use super::*;

/// Represents some particular quantum area of space. The actual DhtLocation that this
/// coordinate corresponds to depends upon the space quantum size specified
/// in the Topology
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
    derive_more::Sub,
    derive_more::Display,
    derive_more::From,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct SpaceQuantum(u32);

impl SpaceQuantum {
    /// The inclusive locations at either end of this quantum
    pub fn to_loc_bounds(&self, topo: &Topology) -> (Loc, Loc) {
        let (a, b): (u32, u32) = bounds(&topo.space, 0, self.0.into(), 1);
        (Loc::from(a), Loc::from(b))
    }
}

/// Represents some particular quantum area of time . The actual Timestamp that this
/// coordinate corresponds to depends upon the time quantum size specified
/// in the Topology
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
    derive_more::Sub,
    derive_more::Display,
    derive_more::From,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct TimeQuantum(u32);

impl TimeQuantum {
    /// The quantum which contains this timestamp
    pub fn from_timestamp(topo: &Topology, timestamp: Timestamp) -> Self {
        topo.time_quantum(timestamp)
    }

    /// The inclusive timestamps at either end of this quantum
    pub fn to_timestamp_bounds(&self, topo: &Topology) -> (Timestamp, Timestamp) {
        let (a, b): (i64, i64) = bounds64(&topo.time, 0, self.0.into(), 1);
        (
            Timestamp::from_micros(a + topo.time_origin.as_micros()),
            Timestamp::from_micros(b + topo.time_origin.as_micros()),
        )
    }
}

/// A quantum in the physical sense: the smallest possible amount of something.
/// Here, we are talking about Time and Space quanta.
pub trait Quantum:
    Copy + Clone + From<u32> + PartialEq + Eq + PartialOrd + Ord + std::fmt::Debug
{
    /// The absolute coordinate which this quantum corresponds to (time or space)
    type Absolute;

    /// The u32 representation
    fn inner(&self) -> u32;

    /// Return the proper dimension (time or space) from the topology
    fn dimension(topo: &Topology) -> &Dimension;

    /// If this coord is beyond the max value for its dimension, wrap it around
    /// the max value
    fn normalized(self, topo: &Topology) -> Self;

    /// The maximum quantum for this dimension
    fn max_value(topo: &Topology) -> Self {
        Self::from((2u64.pow(Self::dimension(topo).bit_depth as u32) - 1) as u32)
    }

    /// Convert to the absolute u32 coordinate space, wrapping if needed
    fn exp_wrapping(&self, topo: &Topology, pow: u8) -> u32 {
        (self.inner() as u64 * Self::dimension(topo).quantum as u64 * 2u64.pow(pow as u32)) as u32
    }

    /// Exposes wrapping addition for the u32
    fn wrapping_add(self, other: u32) -> Self {
        Self::from((self.inner()).wrapping_add(other))
    }

    /// Exposes wrapping subtraction for the u32
    fn wrapping_sub(self, other: u32) -> Self {
        Self::from((self.inner()).wrapping_sub(other))
    }
}

impl Quantum for SpaceQuantum {
    type Absolute = Loc;

    fn inner(&self) -> u32 {
        self.0
    }

    fn dimension(topo: &Topology) -> &Dimension {
        &topo.space
    }

    fn normalized(self, topo: &Topology) -> Self {
        let depth = topo.space.bit_depth;
        if depth >= 32 {
            self
        } else {
            Self(self.0 % pow2(depth))
        }
    }
}

impl Quantum for TimeQuantum {
    type Absolute = Timestamp;

    fn inner(&self) -> u32 {
        self.0
    }

    fn dimension(topo: &Topology) -> &Dimension {
        &topo.time
    }

    // Time coordinates do not wrap, so normalization is an identity
    fn normalized(self, _topo: &Topology) -> Self {
        self
    }
}

/// A SpaceQuantum and a TimeQuantum form a quantum of spacetime
#[derive(Debug)]
pub struct SpacetimeQuantumCoords {
    /// The space quantum coordinate
    pub space: SpaceQuantum,
    /// The time quantum coordinate
    pub time: TimeQuantum,
}

impl SpacetimeQuantumCoords {
    /// Unpack the space and time coordinates
    pub fn to_tuple(&self) -> (u32, u32) {
        (self.space.0, self.time.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_bounds_unit_topo() {
        let topo = Topology::unit_zero();

        assert_eq!(
            SpaceQuantum::from(12).to_loc_bounds(&topo),
            (12.into(), 12.into())
        );
        assert_eq!(
            SpaceQuantum::max_value(&topo).to_loc_bounds(&topo),
            (u32::MAX.into(), u32::MAX.into())
        );

        assert_eq!(
            TimeQuantum::from(12).to_timestamp_bounds(&topo),
            (Timestamp::from_micros(12), Timestamp::from_micros(12))
        );

        assert_eq!(
            TimeQuantum::max_value(&topo).to_timestamp_bounds(&topo),
            (
                Timestamp::from_micros(u32::MAX as i64),
                Timestamp::from_micros(u32::MAX as i64),
            )
        );
    }

    #[test]
    fn to_bounds_standard_topo() {
        let origin = Timestamp::ZERO;
        let topo = Topology::standard(origin.clone(), Duration::ZERO);
        let epoch = origin.as_micros();
        let xq = topo.space.quantum;
        let tq = topo.time.quantum as i64;

        assert_eq!(
            SpaceQuantum::from(12).to_loc_bounds(&topo),
            ((12 * xq).into(), (13 * xq - 1).into())
        );
        assert_eq!(
            SpaceQuantum::max_value(&topo).to_loc_bounds(&topo),
            ((u32::MAX - xq + 1).into(), u32::MAX.into())
        );

        assert_eq!(
            TimeQuantum::from(12).to_timestamp_bounds(&topo),
            (
                Timestamp::from_micros(epoch + 12 * tq),
                Timestamp::from_micros(epoch + 13 * tq - 1)
            )
        );

        // just ensure this doesn't panic
        let _ = TimeQuantum::max_value(&topo).to_timestamp_bounds(&topo);
    }

    #[test]
    fn test_contains() {
        let topo = Topology::unit_zero();
        let s = TimeSegment::new(31, 0);
        assert_eq!(s.quantum_bounds(&topo), (0.into(), (u32::MAX / 2).into()));
        assert!(s.contains_quantum(&topo, 0.into()));
        assert!(!s.contains_quantum(&topo, (u32::MAX / 2 + 2).into()));
    }

    #[test]
    fn test_contains_normalized() {
        let topo = Topology::standard_epoch_full();
        let m = pow2(topo.space.bit_depth);
        let s = SpaceSegment::new(2, m + 5);
        let bounds = s.quantum_bounds(&topo);
        // The quantum bounds are normalized (wrapped)
        assert_eq!(bounds, SpaceSegment::new(2, 5).quantum_bounds(&topo));
        assert_eq!(bounds, (20.into(), 23.into()));

        assert!(s.contains_quantum(&topo, 20.into()));
        assert!(s.contains_quantum(&topo, 23.into()));
        assert!(s.contains_quantum(&topo, (m * 2 + 20).into()));
        assert!(s.contains_quantum(&topo, (m * 3 + 23).into()));
        assert!(!s.contains_quantum(&topo, (m * 4 + 24).into()));
    }
}
