use std::{marker::PhantomData, ops::AddAssign};

use crate::op::{Loc, Timestamp};
use derivative::Derivative;
use num_traits::Num;

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
    derive_more::Display,
    derive_more::From,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct SpaceQuantum(u32);

impl SpaceQuantum {
    pub fn to_loc_bounds(&self, topo: &Topology) -> (Loc, Loc) {
        let (a, b) = bounds(self.0, topo.space.quantum);
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
        let (a, b) = bounds(self.0 as i64, topo.time.quantum as i64);
        (
            Timestamp::from_micros(a + topo.time_origin.as_micros()),
            Timestamp::from_micros(b + topo.time_origin.as_micros()),
        )
    }
}

pub trait Quantum: From<u32> + PartialEq + Eq + PartialOrd + Ord + std::fmt::Debug {
    const MAX: u32 = u32::MAX;
    type Target;

    fn inner(&self) -> u32;

    fn size(topo: &Topology) -> u32;

    fn exp(&self, pow: u8) -> u32 {
        self.inner() * 2u32.pow(pow as u32)
    }

    fn exp_wrapping(&self, pow: u8) -> u32 {
        (self.inner() as u64 * 2u64.pow(pow as u32)) as u32
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

    fn size(topo: &Topology) -> u32 {
        topo.space.quantum
    }
}

impl Quantum for TimeQuantum {
    type Target = Timestamp;

    fn inner(&self) -> u32 {
        self.0
    }

    fn size(topo: &Topology) -> u32 {
        topo.time.quantum
    }
}

pub struct SpacetimeCoords {
    pub space: SpaceQuantum,
    pub time: TimeQuantum,
}

impl SpacetimeCoords {
    pub fn to_tuple(&self) -> (u32, u32) {
        (self.space.0, self.time.0)
    }
}

fn bounds<
    N: Copy + Num + num_traits::WrappingAdd + num_traits::WrappingSub + num_traits::WrappingMul,
>(
    offset: N,
    length: N,
) -> (N, N) {
    let m = offset.wrapping_mul(&length);
    (m, m.wrapping_add(&length).wrapping_sub(&N::one()))
}

/// Any interval in space or time is represented by a node in a tree, so our
/// way of describing intervals uses tree coordinates as well:
/// The length of an interval is 2^(power), and the position of its left edge
/// is at (offset * length).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Segment<Q: Quantum> {
    // TODO: make `u8`?
    pub power: u32,
    pub offset: u32,
    phantom: PhantomData<Q>,
}

impl<Q: Quantum> Segment<Q> {
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

    /// Get the quanta which bound this segment
    pub fn quantum_bounds(&self) -> (Q, Q) {
        let (a, b) = bounds(self.offset, self.length() as u32);
        (Q::from(a), Q::from(b))
    }

    pub fn contains(&self, coord: Q) -> bool {
        let (lo, hi) = self.quantum_bounds();
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
                Segment::new(power, self.offset * 2),
                Segment::new(power, self.offset * 2 + 1),
            ))
        }
    }
}

impl SpaceSegment {
    pub fn loc_bounds(&self, topo: &Topology) -> (Loc, Loc) {
        let sz = SpaceQuantum::size(topo);
        let (a, b) = bounds(self.offset, (self.length() as u32).wrapping_mul(sz));
        (Loc::from(a), Loc::from(b))
    }
}

impl TimeSegment {
    pub fn timestamp_bounds(&self, topo: &Topology) -> (Timestamp, Timestamp) {
        let sz = TimeQuantum::size(topo) as i64;
        let (a, b) = bounds(self.offset as i64, (self.length() as i64).wrapping_mul(sz));
        (Timestamp::from_micros(a), Timestamp::from_micros(b))
    }
}

pub type SpaceSegment = Segment<SpaceQuantum>;
pub type TimeSegment = Segment<TimeQuantum>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dimension {
    /// The smallest possible length in this dimension.
    /// Determines the interval represented by the leaf of a tree.
    quantum: u32,
    /// The size of this dimension, meaning the number of possible values
    /// that can be represented.
    ///
    /// Unused, but could be used for a more compact wire data type.
    bit_depth: u8,
}

impl Dimension {
    pub fn identity() -> Self {
        Dimension {
            quantum: 1,
            bit_depth: 32,
        }
    }

    pub const fn standard_space() -> Self {
        let quantum_power = 12;
        Dimension {
            // if a network has 1 million peers, the average spacing between them is ~4,300
            // so at a target coverage of 100, each arc will be ~430,000 in length
            // which divided by 16 is ~2700, which is about 2^15.
            // So, we'll go down to 2^12.
            // This means we only need 20 bits to represent any location.
            quantum: 2u32.pow(quantum_power),
            bit_depth: 20,
        }
    }

    pub const fn standard_time() -> Self {
        Dimension {
            // 5 minutes, in microseconds
            quantum: 1_000_000 * 60 * 5,

            // 12 quanta = 1 hour.
            // If we set the max lifetime for a network to ~100 years, which
            // is 12 * 24 * 365 * 1000 = 105,120,000 time quanta,
            // the log2 of which is 26.64,
            // then we can store any time coordinate in that range using 27 bits.
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
    pub fn identity(time_origin: Timestamp) -> Self {
        Self {
            space: Dimension::identity(),
            time: Dimension::identity(),
            time_origin,
        }
    }

    pub fn identity_zero() -> Self {
        Self {
            space: Dimension::identity(),
            time: Dimension::identity(),
            time_origin: Timestamp::from_micros(0),
        }
    }

    pub fn standard(time_origin: Timestamp) -> Self {
        Self {
            space: Dimension::standard_space(),
            time: Dimension::identity(),
            time_origin,
        }
    }

    pub fn standard_epoch() -> Self {
        Self::standard(Timestamp::HOLOCHAIN_EPOCH)
    }

    pub fn space_coord(&self, x: Loc) -> SpaceQuantum {
        (x.as_u32() / self.space.quantum).into()
    }

    pub fn time_coord(&self, t: Timestamp) -> TimeQuantum {
        let t = (t.as_micros() - self.time_origin.as_micros()).max(0);
        ((t / self.time.quantum as i64) as u32).into()
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
        let zs = now.leading_zeros();
        now <<= zs;
        let mut seg = TimeSegment::new(32 - zs - 1, 0);
        let mut times = vec![];
        let mask = 1u32.rotate_right(1); // 0b100000...
        let iters = 32 - zs - 1;
        let mut max = self.limit.unwrap_or(iters * 2);
        for _ in 0..iters {
            seg.power -= 1;
            seg.offset *= 2;

            // remove the leading zero and shift left
            now &= !mask;
            now <<= 1;

            times.push(seg);
            seg.offset += 1;
            max -= 1;
            if max == 0 {
                break;
            }
            if now & mask > 0 {
                // if the MSB is 1, duplicate the segment
                times.push(seg);
                seg.offset += 1;
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
        let mut lt: Vec<_> = lt.segments().iter().map(TimeSegment::length).collect();
        let rt: Vec<_> = rt.segments().iter().map(TimeSegment::length).collect();
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
    fn test_contains() {
        let s = TimeSegment::new(31, 0);
        assert_eq!(s.quantum_bounds(), (0.into(), (u32::MAX / 2).into()));
        assert!(s.contains(0.into()));
        assert!(!s.contains((u32::MAX / 2 + 2).into()));
    }

    #[test]
    fn segment_length() {
        let s = TimeSegment::new(31, 0);
        assert_eq!(s.length(), 2u64.pow(31));
    }

    fn lengths(t: TimeQuantum) -> Vec<u32> {
        TelescopingTimes::new(t)
            .segments()
            .into_iter()
            .map(|i| i.length() as u32)
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
            let ts = TelescopingTimes::new(now.into()).segments();
            let total = ts.iter().fold(0u64, |len, t| {
                assert_eq!(t.quantum_bounds().0.inner(), len as u32, "t = {:?}, len = {}", t, len);
                len + t.length()
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
