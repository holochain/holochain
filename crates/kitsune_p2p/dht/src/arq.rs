//! "Quantized DHT Arc"

mod arq_set;
mod peer_view;
mod strat;

#[cfg(feature = "testing")]
pub mod ascii;

pub use arq_set::*;

pub use peer_view::*;
pub use strat::*;

use kitsune_p2p_dht_arc::{ArcInterval, DhtArc};

use crate::{op::Loc, quantum::*};

pub fn pow2(p: u8) -> u32 {
    2u32.pow(p as u32)
}

pub fn pow2f(p: u8) -> f64 {
    2f64.powf(p as f64)
}

pub trait ArqBounded: Sized + serde::Serialize + serde::de::DeserializeOwned {
    fn to_interval(&self, topo: &Topology) -> ArcInterval;

    fn absolute_length(&self, topo: &Topology) -> u64;

    fn length_ratio(&self, topo: &Topology) -> f64 {
        self.absolute_length(topo) as f64 / 2f64.powf(32.0)
    }

    /// Get a reference to the arq's power.
    fn power(&self) -> u8;

    /// Get a reference to the arq's count.
    fn count(&self) -> u32;

    /// Requantize to a different power. If requantizing to a higher power,
    /// only requantize if there is no information loss due to rounding.
    /// Otherwise, return None.
    fn requantize(&self, power: u8) -> Option<Self>;

    fn is_full(&self) -> bool {
        is_full(self.power(), self.count())
    }

    fn is_empty(&self) -> bool {
        self.count() == 0
    }

    fn to_bounds(&self) -> ArqBounds;

    fn to_ascii(&self, topo: &Topology, len: usize) -> String {
        self.to_bounds().to_ascii(topo, len)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Arq {
    /// Location around which this coverage is centered
    center: Loc,
    /// The level of quantization. Total ArqBounds length is `2^power * count`.
    /// The power must be between 0 and 31, inclusive.
    power: u8,
    /// The number of unit lengths.
    /// We never expect the count to be less than 4 or so, and not much larger
    /// than 32.
    count: u32,
}

// impl PartialEq for Arq {
//     fn eq(&self, other: &Self) -> bool {
//         let sl = self.spacing();
//         let sr = other.spacing();
//         self.count.wrapping_mul(sl) == other.count.wrapping_mul(sr) && self.center == other.center
//     }
// }

// impl Eq for Arq {}

impl Arq {
    pub fn new(center: Loc, power: u8, count: u32) -> Self {
        Self {
            center,
            power,
            count,
        }
    }

    pub fn new_full(center: Loc, power: u8) -> Self {
        let count = 2u32.pow(32 - power as u32);
        assert!(is_full(power, count));
        Self {
            center,
            power,
            count,
        }
    }

    pub fn spacing(&self) -> u32 {
        2u32.pow(self.power as u32)
    }

    /// Requantize to a different power. If requantizing to a higher power,
    /// only requantize if there is no information loss due to rounding.
    /// Otherwise, return None.
    pub fn requantize(&self, power: u8) -> Option<Self> {
        requantize(self.power, self.count, power).map(|(power, count)| Self {
            center: self.center,
            power,
            count,
        })
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

    /// Calculate chunks at successive distances from the center.
    /// index 0 is the chunk containing the center location.
    /// index 1 is the adjacent chunk to the left or right, depending on the
    ///     center location
    /// index 2 is the chunk on the other side of the center chunk,
    /// and so on.
    ///
    /// In general, the sequence looks like one of the following, depending
    /// on which side of the central chunk the centerpoint is closest to.
    ///         ... 5 3 1 0 2 4 6 ...
    ///                - or -
    ///         ... 6 4 2 0 1 2 3 ...
    fn chunk_at(&self, sequence: u32) -> SpaceSegment {
        let s = self.spacing();
        // the offset of the central chunk
        let center = self.center.as_u32() / s;
        let offset = center.wrapping_add(sequence);
        SpaceSegment::new(self.power.into(), offset)
    }

    /// Return the chunks at the leftmost and rightmost edge of this Arq.
    /// If count is 0, there is no boundary.
    /// If count is 1, both boundary chunks are the same: the central chunk.
    /// Otherwise, returns two different chunks.
    pub fn boundary_chunks(&self) -> Option<(SpaceSegment, SpaceSegment)> {
        if self.count == 0 {
            None
        } else if self.count == 1 {
            let c = self.chunk_at(0);
            Some((c.clone(), c))
        } else {
            Some((self.chunk_at(0), self.chunk_at(self.count - 1)))
        }
    }

    /// Get a reference to the arq's center.
    pub fn center(&self) -> Loc {
        self.center
    }

    /// Get a mutable reference to the arq's count.
    pub fn count_mut(&mut self) -> &mut u32 {
        &mut self.count
    }

    pub fn to_dht_arc(&self, topo: &Topology) -> DhtArc {
        let len = self.absolute_length(topo);
        let hl = ((len + 1) / 2) as u32;
        DhtArc::new(self.center, hl)
    }

    pub fn from_dht_arc(topo: &Topology, strat: &ArqStrat, dht_arc: &DhtArc) -> Self {
        approximate_arq(
            topo,
            strat,
            dht_arc.center_loc(),
            dht_arc.half_length() as u64 * 2,
        )
    }
}

impl From<Arq> for ArqBounds {
    fn from(a: Arq) -> Self {
        a.to_bounds()
    }
}

impl From<&Arq> for ArqBounds {
    fn from(a: &Arq) -> Self {
        a.to_bounds()
    }
}

impl From<&ArqBounds> for ArqBounds {
    fn from(a: &ArqBounds) -> Self {
        *a
    }
}

impl ArqBounded for Arq {
    fn to_bounds(&self) -> ArqBounds {
        let s = self.spacing();
        let c = self.center.as_u32();
        let center_offset = c / s;
        ArqBounds {
            offset: center_offset.into(),
            power: self.power,
            count: self.count,
        }
    }

    fn to_interval(&self, topo: &Topology) -> ArcInterval {
        self.to_bounds().to_interval(topo)
    }

    fn absolute_length(&self, topo: &Topology) -> u64 {
        self.to_interval(topo).length()
    }

    fn power(&self) -> u8 {
        self.power
    }

    fn count(&self) -> u32 {
        self.count
    }

    fn requantize(&self, power: u8) -> Option<Self> {
        requantize(self.power, self.count, power).map(|(power, count)| Self {
            center: self.center,
            power,
            count,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArqBounds {
    offset: SpaceQuantum,
    power: u8,
    count: u32,
}

// impl PartialEq for ArqBounds {
//     fn eq(&self, other: &Self) -> bool {
//         let sl = self.spacing();
//         let sr = other.spacing();
//         self.count == 0 && other.count == 0
//             || (self.count.wrapping_mul(sl) == other.count.wrapping_mul(sr)
//                 && self.offset.inner().wrapping_mul(sl) == other.offset.inner().wrapping_mul(sr))
//     }
// }

// impl Eq for ArqBounds {}

impl ArqBounded for ArqBounds {
    fn to_bounds(&self) -> ArqBounds {
        *self
    }

    fn to_interval(&self, topo: &Topology) -> ArcInterval {
        if is_full(self.power, self.count) {
            ArcInterval::Full
        } else if let Some((a, b)) = self.boundary_chunks() {
            ArcInterval::new(a.left(topo), b.right(topo))
        } else {
            ArcInterval::Empty
        }
    }

    fn absolute_length(&self, topo: &Topology) -> u64 {
        self.to_interval(topo).length()
    }

    fn power(&self) -> u8 {
        self.power
    }

    fn count(&self) -> u32 {
        self.count
    }

    fn requantize(&self, power: u8) -> Option<Self> {
        requantize(self.power, self.count, power).map(|(power, count)| Self {
            offset: self.offset,
            power,
            count,
        })
    }
}

impl ArqBounds {
    pub fn equivalent(topo: &Topology, a: &Self, b: &Self) -> bool {
        let qa = a.spacing() * topo.space.quantum;
        let qb = b.spacing() * topo.space.quantum;
        a.count == 0 && b.count == 0
            || (a.offset.inner().wrapping_mul(qa) == b.offset.inner().wrapping_mul(qb)
                && a.count.wrapping_mul(qa) == b.count.wrapping_mul(qb))
    }

    pub fn from_interval_rounded(topo: &Topology, power: u8, interval: ArcInterval) -> Self {
        Self::from_interval_inner(&topo.space, power, interval, true).unwrap()
    }

    pub fn from_interval(topo: &Topology, power: u8, interval: ArcInterval) -> Option<Self> {
        Self::from_interval_inner(&topo.space, power, interval, false)
    }

    pub fn from_parts(power: u8, offset: SpaceQuantum, count: u32) -> Self {
        Self {
            power,
            offset,
            count,
        }
    }

    pub fn to_arq(&self) -> Arq {
        Arq {
            center: self.pseudocenter(),
            power: self.power,
            count: self.count,
        }
    }

    pub fn empty(power: u8) -> Self {
        Self::from_interval(&Topology::unit_zero(), power, ArcInterval::Empty).unwrap()
    }

    fn from_interval_inner(
        dim: &Dimension,
        power: u8,
        interval: ArcInterval,
        rounded: bool,
    ) -> Option<Self> {
        match interval {
            ArcInterval::Empty => Some(Self {
                offset: 0.into(),
                power,
                count: 0,
            }),
            ArcInterval::Full => {
                assert!(power > 0);
                let full_count = 2u32.pow(32 - power as u32);
                Some(Self {
                    offset: 0.into(),
                    power,
                    count: full_count,
                })
            }
            ArcInterval::Bounded(lo, hi) => {
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
                        offset: offset.into(),
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

    /// Return the chunks at the leftmost and rightmost edge of this Arq.
    /// If count is 0, there is no boundary.
    /// If count is 1, both boundary chunks are the same: the central chunk.
    /// Otherwise, returns two different chunks.
    pub fn boundary_chunks(&self) -> Option<(ArqBounds, ArqBounds)> {
        if self.count == 0 {
            None
        } else if self.count == 1 {
            Some((self.clone(), self.clone()))
        } else {
            let mut a = self.clone();
            let mut b = self.clone();
            a.count = 1;
            b.count = 1;
            b.offset = (b.offset.inner()).wrapping_add(self.count - 1).into();
            Some((a, b))
        }
    }

    pub fn segments(&self) -> impl Iterator<Item = SpaceSegment> + '_ {
        (0..self.count)
            .map(|c| SpaceSegment::new(self.power.into(), c.wrapping_add(self.offset.inner())))
    }

    pub fn chunk_width(&self) -> u64 {
        2u64.pow(self.power as u32)
    }

    // TODO: test
    pub fn left(&self, topo: &Topology) -> u32 {
        self.offset.exp_wrapping(topo, self.power)
    }

    // TODO: test
    // XXX: doesn't really apply for an empty ArqBounds!
    pub fn right(&self, topo: &Topology) -> u32 {
        self.offset
            .wrapping_add(self.count)
            .exp_wrapping(topo, self.power)
            .wrapping_sub(1)
    }

    /// Return a plausible place for the centerpoint of the Arq.
    /// Obviously these pseudo-centerpoints are not evenly distributed, so
    /// be careful where you use them.
    pub fn pseudocenter(&self) -> Loc {
        let s = self.spacing();
        let center = (s * self.offset.inner()).wrapping_add(s / 2);
        Loc::from(center as u32)
    }

    pub fn spacing(&self) -> u32 {
        2u32.pow(self.power as u32)
    }

    /// Get a reference to the arq bounds's count.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Get a reference to the arq bounds's offset.
    pub fn offset(&self) -> SpaceQuantum {
        self.offset
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
pub fn approximate_arq(topo: &Topology, strat: &ArqStrat, center: Loc, len: u64) -> Arq {
    if len == 2u64.pow(32) {
        Arq::new_full(center, strat.max_power)
    } else if len == 0 {
        Arq::new(center, strat.min_power, 0)
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
        Arq::new(center, power as u8, count)
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
        assert_eq!(full1.to_interval(&topo), ArcInterval::Full);
        assert_eq!(full2.to_interval(&topo), ArcInterval::Full);
    }

    #[test]
    fn test_chunk_at() {
        let c = Arq {
            center: Loc::from(256),
            power: 4,
            count: 10,
        };

        assert_eq!(c.chunk_at(0).offset, 16);
        assert_eq!(c.chunk_at(1).offset, 17);
        assert_eq!(c.chunk_at(2).offset, 18);
        assert_eq!(c.chunk_at(3).offset, 19);
    }

    #[test]
    fn arq_requantize() {
        let c = Arq {
            center: Loc::from(42),
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
            center: Loc::from(42),
            power: 20,
            count: 256,
        };

        assert_eq!(rq(&c, 12).map(|c| c.count), Some(256 * 256));
        assert_eq!(rq(&c, 28).map(|c| c.count), Some(1));
        assert_eq!(rq(&c, 29).map(|c| c.count), None);
    }

    #[test]
    fn to_bounds() {
        let pow: u8 = 4;
        {
            let a = Arq::new((2u32.pow(pow.into()) - 1).into(), pow, 16);
            let b = a.to_bounds();
            assert_eq!(b.offset(), SpaceQuantum::from(0));
            assert_eq!(b.count(), 16);
        }
        {
            let a = Arq::new(4.into(), pow, 18);
            let b = a.to_bounds();
            assert_eq!(b.count(), 18);
        }
    }

    #[test]
    fn from_interval_regression() {
        let topo = Topology::unit_zero();
        let i = ArcInterval::Bounded(4294967040u32.into(), 511.into());
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
            let mut bounds: Vec<_> = arqs.into_iter().map(|a| a.to_bounds()).enumerate().collect();

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
            assert_eq!(arq, arq2);
        }

        #[test]
        fn arc_interval_roundtrip(center: u32, pow in 3..19u8, count in 0..8u32) {
            let topo = Topology::standard_epoch();
            let length = count as u64 * 2u64.pow(pow as u32) / 2 * 2;
            let strat = ArqStrat::default();
            let arq = approximate_arq(&topo, &strat, center.into(), length).to_bounds();
            let interval = arq.to_interval(&topo);
            let arq2 = ArqBounds::from_interval(&topo, arq.power(), interval.clone()).unwrap();
            assert!(ArqBounds::equivalent(&topo, &arq, &arq2));
        }
    }
}
