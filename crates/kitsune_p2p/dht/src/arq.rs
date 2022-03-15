//! "Quantized DHT Arc"

mod arq_set;
mod peer_view;
mod strat;

#[cfg(feature = "testing")]
pub mod ascii;

pub use arq_set::*;

pub use peer_view::*;
pub use strat::*;

use kitsune_p2p_dht_arc::DhtArc;

use crate::{op::Loc, quantum::*};

pub fn pow2(p: u8) -> u32 {
    2u32.pow(p as u32)
}

pub fn pow2f(p: u8) -> f64 {
    2f64.powf(p as f64)
}

/// Represents the start point or "left edge" of an Arq.
///
/// This helps us generalize over the two use cases of Arq:
/// 1. An Arq which is defined at a definite absolute DhtLocation corresponding
///    to an Agent's location, and which can be requantized, resized, etc.
/// 2. An Arq which has no absolute location defined, and which simply represents
///    a (quantized) range.
pub trait ArqStart {
    fn to_loc(&self, topo: &Topology, power: u8) -> Loc;
}

impl ArqStart for Loc {
    fn to_loc(&self, _topo: &Topology, _power: u8) -> Loc {
        *self
    }
}

impl ArqStart for Offset {
    fn to_loc(&self, topo: &Topology, power: u8) -> Loc {
        self.wrapping_mul(topo.space.quantum)
            .wrapping_mul(pow2(power))
            .into()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Arq<S: ArqStart = Loc> {
    /// Location around which this coverage is centered
    start: S,
    /// The level of quantization. Total ArqBounds length is `2^power * count`.
    /// The power must be between 0 and 31, inclusive.
    power: u8,
    /// The number of unit lengths.
    /// We never expect the count to be less than 4 or so, and not much larger
    /// than 32.
    count: u32,
}

pub type ArqBounds = Arq<Offset>;

impl<S: ArqStart> Arq<S> {
    /// The number of quanta to use for each segment
    pub fn quantum_spacing(&self) -> u32 {
        pow2(self.power)
    }

    fn absolute_length(&self, topo: &Topology) -> u64 {
        self.to_interval(topo).length()
    }

    fn to_interval(&self, topo: &Topology) -> DhtArc {
        if is_full(self.power, self.count) {
            DhtArc::Full
        } else if count == 0 {
            DhtArc::Empty
        } else {
            DhtArc::from_bounds(a.left(topo), b.right(topo))
        }
    }
    fn power(&self) -> u8 {
        self.power
    }

    fn count(&self) -> u32 {
        self.count
    }

    fn length_ratio(&self, topo: &Topology) -> f64 {
        self.absolute_length(topo) as f64 / 2f64.powf(32.0)
    }

    /// Requantize to a different power. If requantizing to a higher power,
    /// only requantize if there is no information loss due to rounding.
    /// Otherwise, return None.
    pub fn requantize(&self, power: u8) -> Option<Self> {
        requantize(self.power, self.count, power).map(|(power, count)| Self {
            start: self.start,
            power,
            count,
        })
    }

    fn is_full(&self) -> bool {
        is_full(self.power(), self.count())
    }

    fn is_empty(&self) -> bool {
        self.count() == 0
    }

    // fn to_ascii(&self, topo: &Topology, len: usize) -> String {
    //     self.to_bounds(topo).to_ascii(topo, len)
    // }
}

impl Arq<Loc> {
    pub fn new(start: Loc, power: u8, count: u32) -> Self {
        Self {
            start,
            power,
            count,
        }
    }

    pub fn new_full(start: Loc, power: u8) -> Self {
        let count = 2u32.pow(32 - power as u32);
        assert!(is_full(power, count));
        Self {
            start,
            power,
            count,
        }
    }

    pub fn downshift(&self) -> Self {
        let (power, count) = power_downshift(self.power, self.count);
        let mut a = self.clone();
        a.power = power;
        a.count = count;
        a
    }

    pub fn upshift(&self, force: bool) -> Option<Self> {
        let count = if force && self.count % 2 == 1 {
            self.count + 1
        } else {
            self.count
        };
        power_upshift(self.power, count).map(|(power, count)| {
            let mut a = self.clone();
            a.power = power;
            a.count = count;
            a
        })
    }

    fn to_bounds(&self, topo: &Topology) -> ArqBounds {
        ArqBounds {
            start: Offset::from(self.start.as_u32() / self.quantum_spacing() / topo.space.quantum),
            power: self.power,
            count: self.count,
        }
    }

    /// Get a reference to the arq's left edge in absolute coordinates.
    pub fn left_edge(&self) -> Loc {
        self.start
    }

    /// Get a mutable reference to the arq's count.
    pub fn count_mut(&mut self) -> &mut u32 {
        &mut self.count
    }

    pub fn to_dht_arc(&self, topo: &Topology) -> DhtArc {
        let len = self.absolute_length(topo);
        let hl = ((len + 1) / 2) as u32;
        DhtArc::from_start_and_half_len(self.left_edge, hl)
    }

    pub fn from_dht_arc(topo: &Topology, strat: &ArqStrat, dht_arc: &DhtArc) -> Self {
        approximate_arq(
            topo,
            strat,
            dht_arc.start_loc(),
            dht_arc.half_length() as u64 * 2,
        )
    }

    /// The two arqs represent the same interval despite having potentially different terms
    pub fn equivalent(topo: &Topology, a: &Self, b: &Self) -> bool {
        let qa = a.spacing() * topo.space.quantum;
        let qb = b.spacing() * topo.space.quantum;
        a.left_edge == b.left_edge && (a.count.wrapping_mul(qa) == b.count.wrapping_mul(qb))
    }
}

impl From<&ArqBounds> for ArqBounds {
    fn from(a: &ArqBounds) -> Self {
        *a
    }
}

impl ArqBounds {
    /// The two arqs represent the same interval despite having potentially different terms
    pub fn equivalent(topo: &Topology, a: &Self, b: &Self) -> bool {
        let qa = a.spacing() * topo.space.quantum;
        let qb = b.spacing() * topo.space.quantum;
        a.count == 0 && b.count == 0
            || (a.offset.inner().wrapping_mul(qa) == b.offset.inner().wrapping_mul(qb)
                && a.count.wrapping_mul(qa) == b.count.wrapping_mul(qb))
    }

    pub fn from_interval_rounded(topo: &Topology, power: u8, interval: DhtArc) -> Self {
        Self::from_interval_inner(&topo.space, power, interval, true).unwrap()
    }

    pub fn from_interval(topo: &Topology, power: u8, interval: DhtArc) -> Option<Self> {
        Self::from_interval_inner(&topo.space, power, interval, false)
    }

    pub fn from_parts(power: u8, start: Offset, count: u32) -> Self {
        Self {
            power,
            start,
            count,
        }
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn to_arq(&self, topo: &Topology) -> Arq {
        Arq {
            start: self.pseudocenter(topo),
            power: self.power,
            count: self.count,
        }
    }

    pub fn empty(power: u8) -> Self {
        Self::from_interval(&Topology::unit_zero(), power, DhtArc::Empty(todo!())).unwrap()
    }

    fn from_interval_inner(
        dim: &Dimension,
        power: u8,
        interval: DhtArc,
        rounded: bool,
    ) -> Option<Self> {
        match interval {
            DhtArc::Empty(_) => Some(Self {
                start: 0.into(),
                power,
                count: 0,
            }),
            DhtArc::Full(_) => {
                assert!(power > 0);
                let full_count = 2u32.pow(32 - power as u32);
                Some(Self {
                    start: 0.into(),
                    power,
                    count: full_count,
                })
            }
            DhtArc::Bounded(lo, hi) => {
                let lo = lo.as_u32();
                let hi = hi.as_u32();
                let q = dim.quantum;
                let s = 2u32.pow(power as u32) * q;
                let offset = lo / s;
                let len = if lo <= hi {
                    hi - lo + 1
                } else {
                    (2u64.pow(32) - (lo as u64) + (hi as u64) + 1) as u32
                };
                let count = len / s;
                // TODO: this is kinda wrong. The right bound of the interval
                // should be 1 less, but we'll accept if it bleeds over by 1 too.
                if rounded || lo == offset * s && (len % s <= 1) {
                    Some(Self {
                        start: offset.into(),
                        power,
                        count: count.into(),
                    })
                } else {
                    tracing::warn!("{} =?= {} == {} * {}", lo, offset * s, offset, s);
                    tracing::warn!("{} =?= {} == {} * {}", len, count * s, count, s);
                    None
                }
            }
        }
    }

    pub fn segments(&self) -> impl Iterator<Item = SpaceSegment> + '_ {
        (0..self.count)
            .map(|c| SpaceSegment::new(self.power.into(), c.wrapping_add(self.start.inner())))
    }

    pub fn chunk_width(&self) -> u64 {
        2u64.pow(self.power as u32)
    }

    // TODO: test
    pub fn left(&self, topo: &Topology) -> u32 {
        self.start.exp_wrapping(topo, self.power)
    }

    // TODO: test
    // XXX: doesn't really apply for an empty ArqBounds!
    pub fn right(&self, topo: &Topology) -> u32 {
        self.start
            .wrapping_add(self.count)
            .exp_wrapping(topo, self.power)
            .wrapping_sub(1)
    }

    /// Return a plausible place for the centerpoint of the Arq.
    /// Obviously these pseudo-centerpoints are not evenly distributed, so
    /// be careful where you use them.
    pub fn pseudocenter(&self, topo: &Topology) -> Loc {
        let s = self.spacing();
        let center = self
            .start
            .inner()
            .wrapping_mul(s)
            .wrapping_mul(topo.space.quantum)
            .wrapping_add(s / 2);
        Loc::from(center as u32)
    }

    /// Get a reference to the arq bounds's offset.
    pub fn offset(&self) -> SpaceQuantum {
        self.start
    }
}

/// Calculate whether a given combination of power and count corresponds to
/// full DHT coverage
pub fn is_full(power: u8, count: u32) -> bool {
    if power >= 32 {
        true
    } else if power == 0 {
        false
    } else {
        count >= 2u32.pow(32 - power as u32)
    }
}

pub fn requantize(old_power: u8, old_count: u32, new_power: u8) -> Option<(u8, u32)> {
    if old_power < new_power {
        let factor = 2u32.pow((new_power - old_power) as u32);
        let count = old_count / factor;
        if old_count == count * factor {
            Some((new_power, count))
        } else {
            None
        }
    } else {
        let count = old_count * 2u32.pow((old_power - new_power) as u32);
        Some((new_power, count))
    }
}

pub fn power_downshift(power: u8, count: u32) -> (u8, u32) {
    (power - 1, count * 2)
}

pub fn power_upshift(power: u8, count: u32) -> Option<(u8, u32)> {
    if count % 2 == 0 {
        Some((power + 1, count / 2))
    } else {
        None
    }
}

pub fn power_and_count_from_length(dim: &Dimension, len: u64, max_chunks: u32) -> (u8, u32) {
    let mut power = 0;
    let mut count = (len / dim.quantum as u64) as f64;
    let max = max_chunks as f64;

    while count.round() > max {
        power += 1;
        count /= 2.0;
    }
    let count = count.round() as u32;
    (power, count)
}

/// Given a center and a length, give Arq which matches most closely given the provided strategy
pub fn approximate_arq(topo: &Topology, strat: &ArqStrat, start: Loc, len: u64) -> Arq {
    if len == 2u64.pow(32) {
        Arq::new_full(start, strat.max_power)
    } else if len == 0 {
        Arq::new(start, strat.min_power, 0)
    } else {
        let (power, count) = power_and_count_from_length(&topo.space, len, strat.max_chunks());

        let min = strat.min_chunks() as f64;
        let max = strat.max_chunks() as f64;

        debug_assert!(
            power == 0 || count >= min as u32,
            "count < min: {} < {}",
            count,
            min
        );
        debug_assert!(
            power == 0 || count <= max as u32,
            "count > max: {} > {}",
            count,
            max
        );
        debug_assert!(count == 0 || count - 1 <= u32::MAX / topo.space.quantum);
        Arq::new(start, power as u8, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_full() {
        assert!(!is_full(31, 1));
        assert!(is_full(31, 2));
        assert!(is_full(31, 3));

        assert!(!is_full(30, 3));
        assert!(is_full(30, 4));
        assert!(is_full(29, 8));

        assert!(is_full(1, 2u32.pow(31)));
        assert!(!is_full(1, 2u32.pow(31) - 1));
        assert!(is_full(2, 2u32.pow(30)));
        assert!(!is_full(2, 2u32.pow(30) - 1));
    }

    #[test]
    fn test_full_intervals() {
        let topo = Topology::unit_zero();
        let full1 = Arq::new_full(0u32.into(), 29);
        let full2 = Arq::new_full(2u32.pow(31).into(), 25);
        assert!(matches!(full1.to_interval(&topo), DhtArc::Full(_)));
        assert!(matches!(full2.to_interval(&topo), DhtArc::Full(_)));
    }

    #[test]
    fn test_chunk_at() {
        let c = Arq {
            start: Loc::from(256),
            power: 4,
            count: 10,
        };

        assert_eq!(*c.chunk_at(0).offset, 16);
        assert_eq!(*c.chunk_at(1).offset, 17);
        assert_eq!(*c.chunk_at(2).offset, 18);
        assert_eq!(*c.chunk_at(3).offset, 19);
    }

    #[test]
    fn arq_requantize() {
        let c = Arq {
            start: Loc::from(42),
            power: 20,
            count: 10,
        };

        let rq = |c: &Arq, p| (*c).requantize(p);

        assert_eq!(rq(&c, 18).map(|c| c.count), Some(40));
        assert_eq!(rq(&c, 19).map(|c| c.count), Some(20));
        assert_eq!(rq(&c, 20).map(|c| c.count), Some(10));
        assert_eq!(rq(&c, 21).map(|c| c.count), Some(5));
        assert_eq!(rq(&c, 22).map(|c| c.count), None);
        assert_eq!(rq(&c, 23).map(|c| c.count), None);
        assert_eq!(rq(&c, 24).map(|c| c.count), None);

        let c = Arq {
            start: Loc::from(42),
            power: 20,
            count: 256,
        };

        assert_eq!(rq(&c, 12).map(|c| c.count), Some(256 * 256));
        assert_eq!(rq(&c, 28).map(|c| c.count), Some(1));
        assert_eq!(rq(&c, 29).map(|c| c.count), None);
    }

    #[test]
    fn to_bounds() {
        let topo = Topology::unit_zero();
        let pow: u8 = 4;
        {
            let a = Arq::new((2u32.pow(pow.into()) - 1).into(), pow, 16);
            let b = a.to_bounds(&topo);
            assert_eq!(b.offset(), SpaceQuantum::from(0));
            assert_eq!(b.count(), 16);
        }
        {
            let a = Arq::new(4.into(), pow, 18);
            let b = a.to_bounds(&topo);
            assert_eq!(b.count(), 18);
        }
    }

    #[test]
    fn from_interval_regression() {
        let topo = Topology::unit_zero();
        let i = DhtArc::Bounded(4294967040u32.into(), 511.into());
        assert!(ArqBounds::from_interval(&topo, 8, i).is_some());
    }

    proptest::proptest! {
        #[test]
        fn test_preserve_ordering_for_bounds(mut centers: Vec<u32>, count in 0u32..8, power in 10u8..20) {
            let topo = Topology::standard_epoch();

            // given a list of sorted centerpoints
            centers.sort();

            // build identical arqs at each centerpoint and convert them to ArqBounds
            let arqs: Vec<_> = centers.into_iter().map(|c| Arq::new(c.into(), power, count)).collect();
            let mut bounds: Vec<_> = arqs.into_iter().map(|a| a.to_bounds(&topo)).enumerate().collect();

            // Ensure the list of ArqBounds also grows monotonically.
            // However, there may be one point at which monotonicity is broken,
            // corresponding to the left edge wrapping around.
            bounds.sort_by_key(|(_, b)| b.left(&topo));

            let mut prev = 0;
            let mut split = None;
            for (i, (ix, _)) in bounds.iter().enumerate() {
                if prev > *ix {
                    split = Some(i);
                    break;
                }
                prev = *ix;
            }

            // Split the list of bounds in two, if a discontinuity was found,
            // and check the monotonicity of each piece separately.
            let (b1, b2) = bounds.split_at(split.unwrap_or(0));
            let ix1: Vec<_> = b1.iter().map(|(i, _)| i).collect();
            let ix2: Vec<_> = b2.iter().map(|(i, _)| i).collect();
            let mut ix1s = ix1.clone();
            let mut ix2s = ix2.clone();
            ix1s.sort();
            ix2s.sort();
            assert_eq!(ix1, ix1s);
            assert_eq!(ix2, ix2s);
        }

        #[test]
        fn dht_arc_roundtrip_unit_topo(center: u32, pow in 3..29u8, count in 0..8u32) {
            let topo = Topology::unit_zero();
            let length = count as u64 * 2u64.pow(pow as u32) / 2 * 2;
            let strat = ArqStrat::default();
            let arq = approximate_arq(&topo, &strat, center.into(), length);
            let dht_arc = arq.to_dht_arc(&topo);
            let arq2 = Arq::from_dht_arc(&topo, &strat, &dht_arc);
            assert_eq!(arq, arq2);
        }

        #[test]
        fn dht_arc_roundtrip_standard_topo(center: u32, pow in 3..29u8, count in 0..8u32) {
            let topo = Topology::standard_epoch();
            let length = count as u64 * 2u64.pow(pow as u32) / 2 * 2;
            let strat = ArqStrat::default();
            let arq = approximate_arq(&topo, &strat, center.into(), length);
            let dht_arc = arq.to_dht_arc(&topo);
            let arq2 = Arq::from_dht_arc(&topo, &strat, &dht_arc);
            assert!(Arq::equivalent(&topo, &arq, &arq2));
        }

        #[test]
        fn arc_interval_roundtrip(center: u32, pow in 3..19u8, count in 0..8u32) {
            let topo = Topology::standard_epoch();
            let length = count as u64 * 2u64.pow(pow as u32) / 2 * 2;
            let strat = ArqStrat::default();
            let arq = approximate_arq(&topo, &strat, center.into(), length).to_bounds(&topo);
            let interval = arq.to_interval(&topo);
            let arq2 = ArqBounds::from_interval(&topo, arq.power(), interval.clone()).unwrap();
            assert!(ArqBounds::equivalent(&topo, &arq, &arq2));
        }
    }
}
