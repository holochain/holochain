//! Data types representing the various ways space and time can be quantized.
//!
//! Kitsune thinks of space-time coordinates on three different levels:
//!
//! ### Absolute coordinates
//!
//! At the absolute level, space coordinates are represented by `u32` (via `DhtLocation`),
//! and time coordinates by `i64` (via `Timestamp`). The timestamp and DHT location
//! of each op is measured in absolute coordinates, as well as the DHT locations of
//! agents
//!
//! ### Quantized coordinates
//!
//! Some data types represent quantized space/time. The `Topology` for a network
//! determines the quantum size for both the time and space dimensions, meaning
//! that any absolute coordinate will always be a multiple of this quantum size.
//! Hence, quantized coordinates are expressed in terms of multiples of the
//! quantum size.
//!
//! `SpaceQuantum` and `TimeQuantum` express quantized coordinates. They refer
//! to a specific quantum-sized portion of space/time.
//!
//! Note that any transformation between Absolute and Quantized coordinates
//! requires the information contained in the `Topology` of the network.
//!
//! ### Segment coordinates (or, Exponential coordinates)
//!
//! The spacetime we are interested in has dimensions that are not only quantized,
//! but are also hierarchically organized into non-overlapping segments.
//! When expressing segments of space larger than a single quantum, we only ever talk about
//! groupings of 2, 4, 8, 16, etc. quanta at a time, and these groupings are
//! always aligned so that no two segments of a given size ever overlap. Moreover,
//! any two segments of different sizes either overlap completely (one is a strict
//! superset of the other), or they don't overlap at all (they are disjoint sets).
//!
//! Segment coordinates are expressed in terms of:
//! - a *power* (exponent of 2) which determines the length of the segment *expressed as a Quantized coordinate*
//! - an *offset*, which is a multiple of the length of this segment to determine
//!   the "left" edge's distance from the origin *as a Quantized coordinate*
//!
//! You must still convert from these Quantized coordinates to get to the actual
//! Absolute coordinates.
//!
//! The pairing of any `SpaceSegment` with any `TimeSegment` forms a `Region`,
//! a bounded rectangle of spacetime.
//!

use std::{marker::PhantomData, ops::AddAssign};

use crate::{
    op::{Loc, Timestamp},
    prelude::pow2,
    ArqStrat,
};
use derivative::Derivative;

/// Represents some number of space quanta. The actual DhtLocation that this
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
    pub fn to_loc_bounds(&self, topo: &Topology) -> (Loc, Loc) {
        let (a, b): (u32, u32) = bounds(&topo.space, 0, self.0.into(), 1);
        (Loc::from(a), Loc::from(b))
    }
}

/// Represents some number of time quanta. The actual Timestamp that this
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
    pub fn from_timestamp(topo: &Topology, timestamp: Timestamp) -> Self {
        topo.time_coord(timestamp)
    }

    pub fn to_timestamp_bounds(&self, topo: &Topology) -> (Timestamp, Timestamp) {
        let (a, b): (i64, i64) = bounds64(&topo.time, 0, self.0.into(), 1);
        (
            Timestamp::from_micros(a + topo.time_origin.as_micros()),
            Timestamp::from_micros(b + topo.time_origin.as_micros()),
        )
    }
}

pub trait Quantum: From<u32> + PartialEq + Eq + PartialOrd + Ord + std::fmt::Debug {
    type Target;

    fn inner(&self) -> u32;

    fn dimension(topo: &Topology) -> &Dimension;

    /// If this coord is beyond the max value for its dimension, wrap it around
    /// the max value
    fn normalized(self, topo: &Topology) -> Self;

    fn max_value(topo: &Topology) -> Self {
        Self::from((2u64.pow(Self::dimension(topo).bit_depth as u32) - 1) as u32)
    }

    /// Convert to the absolute u32 coordinate space, wrapping if needed
    fn exp_wrapping(&self, topo: &Topology, pow: u8) -> u32 {
        (self.inner() as u64 * Self::dimension(topo).quantum as u64 * 2u64.pow(pow as u32)) as u32
    }

    fn wrapping_add(self, other: u32) -> Self {
        Self::from((self.inner()).wrapping_add(other))
    }

    fn wrapping_sub(self, other: u32) -> Self {
        Self::from((self.inner()).wrapping_sub(other))
    }
}

impl Quantum for SpaceQuantum {
    type Target = Loc;

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
    type Target = Timestamp;

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

#[derive(Debug)]
pub struct SpacetimeCoords {
    pub space: SpaceQuantum,
    pub time: TimeQuantum,
}

impl SpacetimeCoords {
    pub fn to_tuple(&self) -> (u32, u32) {
        (self.space.0, self.time.0)
    }
}

fn bounds<N: From<u32>>(dim: &Dimension, power: u8, offset: Offset, count: u32) -> (N, N) {
    let q = dim.quantum.wrapping_mul(pow2(power));
    let start = offset.wrapping_mul(q);
    let len = count.wrapping_mul(q);
    (start.into(), start.wrapping_add(len).wrapping_sub(1).into())
}

fn bounds64<N: From<i64>>(dim: &Dimension, power: u8, offset: Offset, count: u32) -> (N, N) {
    let q = dim.quantum as i64 * 2i64.pow(power.into());
    let start = (*offset as i64).wrapping_mul(q);
    let len = (count as i64).wrapping_mul(q);
    (start.into(), start.wrapping_add(len).wrapping_sub(1).into())
}

/// An Offset represents the position of the left edge of some Segment.
/// The absolute DhtLocation of the offset is determined by the "power" of its
/// context, and topology of the space, by:
///
///   dht location = offset * 2^pow * topology.space.quantum
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
    derive_more::Mul,
    derive_more::Div,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
    derive_more::Into,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct Offset(pub u32);

impl Offset {
    pub fn to_loc(&self, topo: &Topology, power: u8) -> Loc {
        self.wrapping_mul(topo.space.quantum)
            .wrapping_mul(pow2(power))
            .into()
    }

    pub fn to_quantum(&self, power: u8) -> SpaceQuantum {
        self.wrapping_mul(pow2(power)).into()
    }

    /// Get the nearest rounded-down Offset for this Loc
    pub fn from_loc_rounded(loc: Loc, topo: &Topology, power: u8) -> Offset {
        (loc.as_u32() / topo.space.quantum / pow2(power)).into()
    }
}

/// Any interval in space or time is represented by a node in a tree, so our
/// way of describing intervals uses tree coordinates as well:
/// The length of an interval is 2^(power), and the position of its left edge
/// is at (offset * length).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Segment<Q: Quantum> {
    /// The exponent, where length = 2^power
    pub power: u8,
    /// The offset from the origin, measured in number of lengths
    pub offset: Offset,
    phantom: PhantomData<Q>,
}

impl<Q: Quantum> Segment<Q> {
    pub fn new<O: Into<Offset>>(power: u8, offset: O) -> Self {
        Self {
            power,
            offset: offset.into(),
            phantom: PhantomData,
        }
    }

    pub fn num_quanta(&self) -> u64 {
        // If power is 32, this overflows a u32
        2u64.pow(self.power.into())
    }

    pub fn absolute_length(&self, topo: &Topology) -> u64 {
        let q = Q::dimension(topo).quantum as u64;
        // If power is 32, this overflows a u32
        self.num_quanta() * q
    }

    /// Get the quanta which bound this segment
    pub fn quantum_bounds(&self, topo: &Topology) -> (Q, Q) {
        let n = self.num_quanta();
        let a = (n * u64::from(*self.offset)) as u32;
        (
            Q::from(a).normalized(&topo),
            Q::from(a.wrapping_add(n as u32).wrapping_sub(1)).normalized(&topo),
        )
    }

    pub fn contains(&self, topo: &Topology, coord: Q) -> bool {
        let (lo, hi) = self.quantum_bounds(topo);
        let coord = coord.normalized(&topo);
        if lo <= hi {
            lo <= coord && coord <= hi
        } else {
            lo <= coord || coord <= hi
        }
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
                Segment::new(power, Offset(*self.offset * 2)),
                Segment::new(power, Offset(*self.offset * 2 + 1)),
            ))
        }
    }
}

#[test]
fn test_quantum_bounds() {}

impl SpaceSegment {
    pub fn loc_bounds(&self, topo: &Topology) -> (Loc, Loc) {
        let (a, b): (u32, u32) = bounds(&topo.space, self.power, self.offset, 1);
        (Loc::from(a), Loc::from(b))
    }
}

impl TimeSegment {
    pub fn timestamp_bounds(&self, topo: &Topology) -> (Timestamp, Timestamp) {
        let (a, b): (i64, i64) = bounds64(&topo.time, self.power, self.offset, 1);
        let o = topo.time_origin.as_micros();
        (Timestamp::from_micros(a + o), Timestamp::from_micros(b + o))
    }
}

pub type SpaceSegment = Segment<SpaceQuantum>;
pub type TimeSegment = Segment<TimeQuantum>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dimension {
    /// The smallest possible length in this dimension.
    /// Determines the interval represented by the leaf of a tree.
    pub quantum: u32,

    /// The smallest power of 2 which is larger than the quantum.
    /// Needed for various calculations.
    pub quantum_power: u8,

    /// The log2 size of this dimension, so that 2^bit_depth is the number of
    /// possible values that can be represented.
    bit_depth: u8,
}

impl Dimension {
    pub fn unit() -> Self {
        Dimension {
            quantum: 1,
            quantum_power: 0,
            bit_depth: 32,
        }
    }

    pub const fn standard_space() -> Self {
        let quantum_power = 12;
        Dimension {
            // if a network has 1 million peers,
            // the average spacing between them is ~4,300
            // so at a target coverage of 100,
            // each arc will be ~430,000 in length
            // which divided by 16 (max chunks) is ~2700, which is about 2^15.
            // So, we'll go down to 2^12 just to be extra safe.
            // This means we only need 20 bits to represent any location.
            quantum: 2u32.pow(quantum_power as u32),
            quantum_power,
            bit_depth: 32 - quantum_power,
        }
    }

    pub const fn standard_time() -> Self {
        Dimension {
            // 5 minutes in microseconds = 1mil * 60 * 5 = 300,000,000
            // log2 of this is 28.16, FYI
            quantum: 1_000_000 * 60 * 5,
            quantum_power: 29,

            // 12 quanta = 1 hour.
            // If we set the max lifetime for a network to ~100 years, which
            // is 12 * 24 * 365 * 1000 = 105,120,000 time quanta,
            // the log2 of which is 26.64,
            // then we can store any time coordinate in that range using 27 bits.
            //
            // BTW, the log2 of 100 years in microseconds is 54.81
            bit_depth: 27,
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
    pub fn unit(time_origin: Timestamp) -> Self {
        Self {
            space: Dimension::unit(),
            time: Dimension::unit(),
            time_origin,
        }
    }

    pub fn unit_zero() -> Self {
        Self {
            space: Dimension::unit(),
            time: Dimension::unit(),
            time_origin: Timestamp::from_micros(0),
        }
    }

    pub fn standard(time_origin: Timestamp) -> Self {
        Self {
            space: Dimension::standard_space(),
            time: Dimension::standard_time(),
            time_origin,
        }
    }

    pub fn standard_epoch() -> Self {
        Self::standard(Timestamp::HOLOCHAIN_EPOCH)
    }

    pub fn standard_zero() -> Self {
        Self::standard(Timestamp::ZERO)
    }

    pub fn space_coord(&self, x: Loc) -> SpaceQuantum {
        (x.as_u32() / self.space.quantum).into()
    }

    pub fn time_coord(&self, t: Timestamp) -> TimeQuantum {
        let t = (t.as_micros() - self.time_origin.as_micros()).max(0);
        ((t / self.time.quantum as i64) as u32).into()
    }

    /// The minimum power to use in "exponentional coordinates".
    pub fn min_space_power(&self) -> u8 {
        // if space quantum power is 0, then min has to be at least 1.
        // otherwise, it can be 0
        1u8.saturating_sub(self.space.quantum_power)
    }

    /// The maximum power to use in "exponentional coordinates".
    /// This is 17 for standard space topology. (32 - 12 - 3)
    pub fn max_space_power(&self, strat: &ArqStrat) -> u8 {
        32 - self.space.quantum_power - strat.max_chunks_log2()
    }
}

/// A type which generates a list of exponentially expanding time windows, as per
/// this document: https://hackmd.io/@hololtd/r1IAIbr5Y
#[derive(Copy, Clone, Debug, PartialEq, Eq, Derivative, serde::Serialize, serde::Deserialize)]
#[derivative(PartialOrd, Ord)]
pub struct TelescopingTimes {
    time: TimeQuantum,

    #[derivative(PartialOrd = "ignore")]
    #[derivative(Ord = "ignore")]
    limit: Option<u32>,
}

impl TelescopingTimes {
    pub fn empty() -> Self {
        Self {
            time: 0.into(),
            limit: None,
        }
    }

    pub fn new(time: TimeQuantum) -> Self {
        Self { time, limit: None }
    }

    /// Calculate the exponentially expanding time segments using the binary
    /// representation of the current timestamp.
    ///
    /// The intuition for this algorithm is that the position of the most
    /// significant 1 represents the power of the largest, leftmost time segment,
    /// and subsequent bits represent the powers of 2 below that one.
    /// After the MSB, a 0 represents a single value of the power represented
    /// by that bit, and a 1 represents two values of the power at that bit.
    ///
    /// See the test below which has the first 16 time segments, alongside
    /// the binary representation of the timestamp (+1) which generated,
    /// which illustrates this pattern.
    pub fn segments(&self) -> Vec<TimeSegment> {
        let mut now: u32 = self.time.inner() + 1;
        if now == 1 {
            return vec![];
        }
        let zs = now.leading_zeros() as u8;
        now <<= zs;
        let iters = 32 - zs - 1;
        let mut max = self.limit.unwrap_or(u32::from(iters) * 2);
        if max == 0 {
            return vec![];
        }
        let mut seg = TimeSegment::new(iters, 0);
        let mut times = vec![];
        let mask = 1u32.rotate_right(1); // 0b100000...
        for _ in 0..iters {
            seg.power -= 1;
            *seg.offset *= 2;

            // remove the leading zero and shift left
            now &= !mask;
            now <<= 1;

            times.push(seg);
            *seg.offset += 1;
            max -= 1;
            if max == 0 {
                break;
            }
            if now & mask > 0 {
                // if the MSB is 1, duplicate the segment
                times.push(seg);
                *seg.offset += 1;
                max -= 1;
                if max == 0 {
                    break;
                }
            }
        }
        if self.limit.is_none() {
            // Should be all zeroes at this point
            debug_assert_eq!(now & !mask, 0)
        }
        times
    }

    pub fn limit(&self, limit: u32) -> Self {
        Self {
            time: self.time,
            limit: Some(limit),
        }
    }

    pub fn rectify<T: AddAssign>(a: (&Self, &mut Vec<T>), b: (&Self, &mut Vec<T>)) {
        let (left, right) = if a.0.time > b.0.time { (b, a) } else { (a, b) };
        let (lt, ld) = left;
        let (rt, rd) = right;
        let mut lt: Vec<_> = lt.segments().iter().map(TimeSegment::num_quanta).collect();
        let rt: Vec<_> = rt.segments().iter().map(TimeSegment::num_quanta).collect();
        assert_eq!(lt.len(), ld.len());
        assert_eq!(rt.len(), rd.len());
        let mut i = 0;
        while i < lt.len() - 1 {
            while lt[i] < rt[i] && i < lt.len() - 1 {
                lt[i] += lt.remove(i + 1);
                let d = ld.remove(i + 1);
                ld[i] += d;
            }
            i += 1;
        }
        rd.truncate(ld.len());
    }
}

#[derive(Copy, Clone, Debug, derive_more::Constructor)]
pub struct GossipParams {
    /// What +/- coordinate offset will you accept for timestamps?
    /// e.g. if the time quantum is 5 min,
    /// a time buffer of 2 will allow +/- 10 min.
    ///
    /// This, along with `max_space_power_offset`, determines what range of
    /// region resolution gets stored in the 2D Fenwick tree
    pub max_time_offset: TimeQuantum,

    /// What difference in power will you accept for other agents' Arqs?
    /// e.g. if the power I use in my arq is 22, and this offset is 2,
    /// I won't talk to anyone whose arq is expressed with a power lower
    /// than 20 or greater than 24
    ///
    /// This, along with `max_time_offset`, determines what range of
    /// region resolution gets stored in the 2D Fenwick tree
    pub max_space_power_offset: u8,
}

impl GossipParams {
    pub fn zero() -> Self {
        Self {
            max_time_offset: 0.into(),
            max_space_power_offset: 0,
        }
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
        let topo = Topology::standard(origin.clone());
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
        assert!(s.contains(&topo, 0.into()));
        assert!(!s.contains(&topo, (u32::MAX / 2 + 2).into()));
    }

    #[test]
    fn test_contains_normalized() {
        let topo = Topology::standard_epoch();
        let m = pow2(topo.space.bit_depth);
        let s = SpaceSegment::new(2, m + 5);
        let bounds = s.quantum_bounds(&topo);
        // The quantum bounds are normalized (wrapped)
        assert_eq!(bounds, SpaceSegment::new(2, 5).quantum_bounds(&topo));
        assert_eq!(bounds, (20.into(), 23.into()));

        assert!(s.contains(&topo, 20.into()));
        assert!(s.contains(&topo, 23.into()));
        assert!(s.contains(&topo, (m * 2 + 20).into()));
        assert!(s.contains(&topo, (m * 3 + 23).into()));
        assert!(!s.contains(&topo, (m * 4 + 24).into()));
    }

    #[test]
    fn segment_length() {
        let s = TimeSegment::new(31, 0);
        assert_eq!(s.num_quanta(), 2u64.pow(31));
    }

    fn lengths(t: TimeQuantum) -> Vec<u32> {
        TelescopingTimes::new(t)
            .segments()
            .into_iter()
            .map(|i| i.num_quanta() as u32)
            .collect()
    }

    #[test]
    fn test_telescoping_times_limit() {
        let tt = TelescopingTimes::new(64.into());
        assert_eq!(tt.segments().len(), 7);
        assert_eq!(tt.limit(6).segments().len(), 6);
        assert_eq!(tt.limit(4).segments().len(), 4);
        assert_eq!(
            tt.segments().into_iter().take(6).collect::<Vec<_>>(),
            tt.limit(6).segments()
        );
    }

    #[test]
    #[rustfmt::skip]
    fn test_telescoping_times_first_16() {
        let ts = TimeQuantum::from;

                                                             // n+1
        assert_eq!(lengths(ts(0)),  Vec::<u32>::new());      // 0001
        assert_eq!(lengths(ts(1)),  vec![1]);                // 0010
        assert_eq!(lengths(ts(2)),  vec![1, 1]);             // 0011
        assert_eq!(lengths(ts(3)),  vec![2, 1]);             // 0100
        assert_eq!(lengths(ts(4)),  vec![2, 1, 1]);          // 0101
        assert_eq!(lengths(ts(5)),  vec![2, 2, 1]);          // 0110
        assert_eq!(lengths(ts(6)),  vec![2, 2, 1, 1]);       // 0111
        assert_eq!(lengths(ts(7)),  vec![4, 2, 1]);          // 1000
        assert_eq!(lengths(ts(8)),  vec![4, 2, 1, 1]);       // 1001
        assert_eq!(lengths(ts(9)),  vec![4, 2, 2, 1]);       // 1010
        assert_eq!(lengths(ts(10)), vec![4, 2, 2, 1, 1]);    // 1011
        assert_eq!(lengths(ts(11)), vec![4, 4, 2, 1]);       // 1100
        assert_eq!(lengths(ts(12)), vec![4, 4, 2, 1, 1]);    // 1101
        assert_eq!(lengths(ts(13)), vec![4, 4, 2, 2, 1]);    // 1110
        assert_eq!(lengths(ts(14)), vec![4, 4, 2, 2, 1, 1]); // 1111
        assert_eq!(lengths(ts(15)), vec![8, 4, 2, 1]);      // 10000
    }

    /// Test that data generated by two different telescoping time sets can be
    /// rectified.
    ///
    /// The data used in this test are simple vecs of integers, but in the real
    /// world, the data would be the region data (which has an AddAssign impl).
    #[test]
    fn test_rectify_telescoping_times() {
        {
            let a = TelescopingTimes::new(5.into());
            let b = TelescopingTimes::new(8.into());

            // the actual integers used here don't matter,
            // they're just picked so that sums look distinct
            let mut da = vec![16, 8, 4];
            let mut db = vec![32, 16, 8, 4];
            TelescopingTimes::rectify((&a, &mut da), (&b, &mut db));
            assert_eq!(da, vec![16 + 8, 4]);
            assert_eq!(db, vec![32, 16]);
        }
        {
            let a = TelescopingTimes::new(14.into());
            let b = TelescopingTimes::new(16.into());
            let mut da = vec![128, 64, 32, 16, 8, 4];
            let mut db = vec![32, 16, 8, 4, 1];
            TelescopingTimes::rectify((&a, &mut da), (&b, &mut db));
            assert_eq!(da, vec![128 + 64, 32 + 16, 8 + 4]);
            assert_eq!(db, vec![32, 16, 8]);
        }
    }

    proptest::proptest! {
        #[test]
        fn telescoping_times_cover_total_time_span(now in 0u32..u32::MAX) {
            let topo = Topology::unit_zero();
            let ts = TelescopingTimes::new(now.into()).segments();
            let total = ts.iter().fold(0u64, |len, t| {
                assert_eq!(t.quantum_bounds(&topo).0.inner(), len as u32, "t = {:?}, len = {}", t, len);
                len + t.num_quanta()
            });
            assert_eq!(total, now as u64);
        }

        #[test]
        fn telescoping_times_end_with_1(now: u32) {
            if let Some(last) = TelescopingTimes::new(now.into()).segments().pop() {
                assert_eq!(last.power, 0);
            }
        }

        #[test]
        fn telescoping_times_are_fractal(now: u32) {
            let a = lengths(now.into());
            let b = lengths((now - a[0]).into());
            assert_eq!(b.as_slice(), &a[1..]);
        }

        #[test]
        fn rectification_doesnt_panic(a: u32, b: u32) {
            let (a, b) = if a < b { (a, b)} else {(b, a)};
            let a = TelescopingTimes::new(a.into());
            let b = TelescopingTimes::new(b.into());
            let mut da = vec![1; a.segments().len()];
            let mut db = vec![1; b.segments().len()];
            TelescopingTimes::rectify((&a, &mut da), (&b, &mut db));
            assert_eq!(da.len(), db.len());
        }
    }
}
