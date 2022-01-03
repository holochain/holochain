//! "Quantized DHT Arc"

mod arq_set;
mod peer_view;
mod strat;

use std::num::Wrapping;

pub use arq_set::*;
pub use peer_view::*;
pub use strat::*;

use kitsune_p2p_dht_arc::ArcInterval;

use crate::op::Loc;

pub fn pow2(p: u8) -> u32 {
    2u32.pow(p as u32)
}

pub fn pow2f(p: u8) -> f64 {
    2f64.powf(p as f64)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arq {
    /// Location around which this coverage is centered
    center: Loc,
    /// The level of quantization. Total length is `2^grid * count`.
    /// The power must be between 0 and 31, inclusive.
    power: u8,
    /// The number of unit lengths.
    /// We never expect the count to be less than 4 or so, and not much larger
    /// than 32.
    count: u32,
}

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
    pub fn requantize(&mut self, power: u8) -> bool {
        requantize(self.power, self.count, power)
            .map(|(power, count)| {
                self.power = power;
                self.count = count;
            })
            .is_some()
    }

    pub fn to_bounds(&self) -> ArqBounds {
        let s = self.spacing();
        let c = self.center.as_u32();
        let center_offset = c / s;
        let left_oriented = c - center_offset * s < s / 2;
        let wing = if left_oriented {
            self.count / 2
        } else {
            (self.count.saturating_sub(1)) / 2
        };
        let offset = if self.count == 0 {
            center_offset
        } else {
            center_offset.wrapping_sub(wing)
        };
        ArqBounds {
            offset,
            power: self.power,
            count: self.count,
        }
    }

    pub fn to_interval(&self) -> ArcInterval {
        self.to_bounds().to_interval()
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
    fn chunk_at(&self, sequence: u32) -> ArqBounds {
        let s = self.spacing();
        // the offset of the central chunk
        let center = self.center.as_u32() / s;
        let left_oriented = (*self.center - Wrapping(center * s)) < Wrapping(s / 2);
        let offset = if left_oriented {
            if sequence % 2 == 1 {
                center.wrapping_sub((sequence / 2 + 1))
            } else {
                center.wrapping_add(sequence / 2)
            }
        } else {
            if sequence % 2 == 1 {
                center.wrapping_add(sequence / 2 + 1)
            } else {
                center.wrapping_sub(sequence / 2)
            }
        };
        ArqBounds::chunk(self.power, offset)
    }

    /// Return the chunks at the leftmost and rightmost edge of this Arq.
    /// If count is 0, there is no boundary.
    /// If count is 1, both boundary chunks are the same: the central chunk.
    /// Otherwise, returns two different chunks.
    pub fn boundary_chunks(&self) -> Option<(ArqBounds, ArqBounds)> {
        if self.count == 0 {
            None
        } else if self.count == 1 {
            let c = self.chunk_at(0);
            Some((c.clone(), c))
        } else {
            let a = self.chunk_at(self.count - 2);
            let b = self.chunk_at(self.count - 1);
            if a.offset < b.offset {
                Some((a, b))
            } else {
                Some((b, a))
            }
        }
    }

    /// Get a reference to the arq's center.
    pub fn center(&self) -> Loc {
        self.center
    }

    /// Get a reference to the arq's power.
    pub fn power(&self) -> u8 {
        self.power
    }

    /// Get a reference to the arq's count.
    pub fn count(&self) -> u32 {
        self.count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArqBounds {
    offset: u32,
    power: u8,
    count: u32,
}

impl ArqBounds {
    pub fn from_interval_rounded(power: u8, interval: ArcInterval) -> Self {
        Self::from_interval_inner(power, interval, true).unwrap()
    }

    pub fn from_interval(power: u8, interval: ArcInterval) -> Option<Self> {
        Self::from_interval_inner(power, interval, false)
    }

    fn from_interval_inner(power: u8, interval: ArcInterval, rounded: bool) -> Option<Self> {
        assert!(power > 0);
        let full_count = 2u32.pow(32 - power as u32);
        match interval {
            ArcInterval::Empty => Some(Self {
                offset: 0,
                power,
                count: 0,
            }),
            ArcInterval::Full => Some(Self {
                offset: 0,
                power,
                count: full_count,
            }),
            ArcInterval::Bounded(lo, hi) => {
                let lo = lo.as_u32();
                let hi = hi.as_u32();
                let s = 2u32.pow(power as u32);
                let offset = lo / s;
                let diff = if lo <= hi {
                    hi - lo
                } else {
                    (2u64.pow(32) - (hi as u64) + (lo as u64) + 1) as u32
                };
                let count = diff / s;
                // TODO: this is kinda wrong. The right bound of the interval
                // should be 1 less.
                if rounded || lo == offset * s && diff == count * s {
                    Some(Self {
                        offset,
                        power,
                        count,
                    })
                } else {
                    None
                }
            }
        }
    }

    pub fn to_interval(&self) -> ArcInterval {
        if is_full(self.power, self.count) {
            ArcInterval::Full
        } else if let Some((a, b)) = self.boundary_chunks() {
            ArcInterval::new(a.left(), b.right())
        } else {
            ArcInterval::Empty
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
            b.offset = b.offset.wrapping_add(self.count - 1);
            Some((a, b))
        }
    }

    /// Requantize to a different power. If requantizing to a higher power,
    /// only requantize if there is no information loss due to rounding.
    /// Otherwise, return None.
    pub fn requantize(&self, power: u8) -> Option<Self> {
        requantize(self.power, self.count, power).map(|(power, count)| Self {
            offset: self.offset,
            power,
            count,
        })
    }

    pub fn chunk(power: u8, offset: u32) -> Self {
        Self {
            power,
            offset,
            count: 1,
        }
    }

    pub fn chunk_width(&self) -> u64 {
        2u64.pow(self.power as u32)
    }

    // TODO: test
    pub fn left(&self) -> u32 {
        (self.offset as u64 * 2u64.pow(self.power as u32)) as u32
    }

    // TODO: test
    pub fn right(&self) -> u32 {
        ((self.offset.wrapping_add(self.count)) as u64 * 2u64.pow(self.power as u32))
            .wrapping_sub(1) as u32
    }

    /// Return a plausible place for the centerpoint of the Arq.
    /// Obviously these pseudo-centerpoints are not evenly distributed, so
    /// be careful where you use them.
    pub fn pseudocenter(&self) -> Loc {
        let left = self.left() as u64;
        let mut right = self.right() as u64;
        if right < left {
            right += 2u64.pow(32);
        }
        Loc::from(((right - left) / 2) as u32)
    }

    /// Get a reference to the arq bounds's count.
    pub fn count(&self) -> u32 {
        self.count
    }
}

/// Calculate whether a given combination of power and count corresponds to
/// full DHT coverage
pub fn is_full(power: u8, count: u32) -> bool {
    if power >= 32 {
        true
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
        let full1 = Arq::new_full(0.into(), 29);
        let full2 = Arq::new_full(2u32.pow(31).into(), 25);
        assert_eq!(full1.to_interval(), ArcInterval::Full);
        assert_eq!(full2.to_interval(), ArcInterval::Full);
    }

    #[test]
    fn test_chunk_at() {
        let c = Arq {
            center: Loc::from(256),
            power: 4,
            count: 10,
        };

        assert_eq!(c.chunk_at(0).offset, 16);
        assert_eq!(c.chunk_at(1).offset, 15);
        assert_eq!(c.chunk_at(2).offset, 17);
        assert_eq!(c.chunk_at(3).offset, 14);
    }

    /// A function to help make it clearer how an expected ArcInterval
    /// is being constructed:
    /// - p: the power
    /// - e: the left edge of the center chunk
    /// - l: how many chunks are to the left of `e`
    /// - r: how many chunks are to the right of `e`
    fn arc_interval_helper(p: u8, e: u32, l: u32, r: u32) -> ArcInterval {
        ArcInterval::new(
            e.wrapping_sub(2u32.pow(p as u32) * l),
            e.wrapping_add(2u32.pow(p as u32) * r).wrapping_sub(1),
        )
    }

    #[test]
    fn test_interval_progression_left_oriented() {
        let power = 2;
        let mut a = Arq {
            center: Loc::from(41),
            power,
            count: 0,
        };

        let ih = |e, l, r| arc_interval_helper(power, e, l, r);

        assert_eq!(a.to_interval(), ArcInterval::Empty);

        a.count = 1;
        assert_eq!(a.to_interval(), ih(40, 0, 1));

        a.count = 2;
        assert_eq!(a.to_interval(), ih(40, 1, 1));

        a.count = 3;
        assert_eq!(a.to_interval(), ih(40, 1, 2));

        a.count = 4;
        assert_eq!(a.to_interval(), ih(40, 2, 2));

        a.count = 5;
        assert_eq!(a.to_interval(), ih(40, 2, 3));

        a.count = 33;
        // the left edge overflows
        assert_eq!(a.to_interval(), ih(40, 16, 17));

        let c = u32::MAX - 41 + 1;
        let r = u32::MAX - 44 + 1;
        // the right edge overflows
        a.center = Loc::from(c);
        assert_eq!(a.to_interval(), ih(r, 16, 17));
    }

    #[test]
    fn test_interval_progression_right_oriented() {
        let power = 2;
        let mut a = Arq {
            center: Loc::from(42),
            power,
            count: 0,
        };

        let ih = |e, l, r| arc_interval_helper(power, e, l, r);

        assert_eq!(a.to_interval(), ArcInterval::Empty);

        a.count = 1;
        assert_eq!(a.to_interval(), ih(40, 0, 1));

        a.count = 2;
        assert_eq!(a.to_interval(), ih(40, 0, 2));

        a.count = 3;
        assert_eq!(a.to_interval(), ih(40, 1, 2));

        a.count = 4;
        assert_eq!(a.to_interval(), ih(40, 1, 3));

        a.count = 5;
        assert_eq!(a.to_interval(), ih(40, 2, 3));

        a.count = 33;
        // the left edge overflows
        assert_eq!(a.to_interval(), ih(40, 16, 17));

        let c = u32::MAX - 42 + 1;
        let r = u32::MAX - 44 + 1;
        // the right edge overflows
        a.center = Loc::from(c);
        assert_eq!(a.to_interval(), ih(r, 16, 17));
    }

    #[test]
    fn arq_center_parity() {
        // An odd chunk count leads to the same number of chunks around the central chunk.
        let mut c = Arq {
            center: Loc::from(42),
            power: 2,
            count: 5,
        };

        let ih = |e, l, r| arc_interval_helper(2, e, l, r);

        assert_eq!(c.to_interval(), ih(40, 2, 3));

        // An even chunk count leads to the new chunk being added to the right
        // in this case, since 42 is closer to the right edge of its containing
        // chunk (43) than to the left edge (40)
        c.count = 6;
        assert_eq!(c.to_interval(), ih(40, 2, 4));

        // If the center is shifted by 1, then the opposite is true.
        c.center = Loc::from(*c.center - Wrapping(1));
        assert_eq!(c.to_interval(), ih(40, 3, 3));
    }

    #[test]
    fn arq_requantize() {
        let c = Arq {
            center: Loc::from(42),
            power: 20,
            count: 10,
        };

        let rq = |c: &Arq, p| {
            let mut c = c.clone();
            let ok = c.requantize(p);
            ok.then(|| c)
        };

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
    fn to_bounds_regression() {
        // 3264675840 -> 3264675840
        // 3264708608 -> 3264675840
        let a1 = Arq::new(3264675840u32.into(), 16, 6);
        // let a2 = Arq::new((3264675840u32 + 3000).into(), 16, 6);
        let a2 = Arq::new(3264708608u32.into(), 16, 6);
        assert_eq!(a1.to_bounds().offset + 1, a2.to_bounds().offset);
    }

    proptest::proptest! {
        #[test]
        fn test_preserve_ordering_for_bounds(mut centers: Vec<u32>, count in 0u32..8, power in 10u8..20) {
            // given a list of sorted centerpoints
            let n = centers.len();
            centers.sort();

            // build identical arqs at each centerpoint and convert them to ArqBounds
            let arqs: Vec<_> = centers.into_iter().map(|c| Arq::new(c.into(), power, count)).collect();
            let mut bounds: Vec<_> = arqs.into_iter().map(|a| a.to_bounds()).enumerate().collect();

            // Ensure the list of ArqBounds also grows monotonically.
            // However, there may be one point at which monotonicity is broken,
            // corresponding to the left edge wrapping around.
            bounds.sort_by_key(|(_, b)| b.left());

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
    }
}
