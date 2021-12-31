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
        ArqBounds::from_arq(self.clone())
    }

    pub fn to_interval(&self) -> ArcInterval {
        self.to_bounds().to_interval()
    }

    /// true if the centerpoint is closer to the left edge of the central chunk,
    /// false if closer to the right edge.
    fn left_oriented(&self) -> bool {
        let s = Wrapping(self.spacing());
        let left = *self.center / s * s;
        *self.center - left < s / Wrapping(2)
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
    // TODO: test
    pub fn from_arq(arq: Arq) -> Self {
        let s = arq.spacing();
        let left_edge = arq.center.as_u32() / s * s;
        let left_oriented = arq.center.as_u32() - left_edge < s / 2;
        let wing = arq.count as u32 / 2 * s;
        let offset = if arq.count == 0 {
            left_edge
        } else if arq.count % 2 == 0 {
            if left_oriented {
                left_edge.wrapping_sub(wing)
            } else {
                left_edge.wrapping_sub(wing + s)
            }
        } else {
            left_edge.wrapping_sub(wing)
        } / s;
        Self {
            offset: offset,
            power: arq.power,
            count: arq.count,
        }
    }

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
            b.offset += self.count - 1;
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
        ((self.offset.wrapping_add(self.count)) as u64
            * 2u64.pow(self.power as u32).wrapping_sub(1)) as u32
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
    fn test_boundaries() {
        let b = ArqBounds::from_interval(4, ArcInterval::new(-16, 15)).unwrap();
        assert_eq!(b.left(), 0);
        assert_eq!(b.right(), 0);
    }

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

    #[test]
    fn coverage_center_parity() {
        // An odd chunk count leads to the same number of chunks around the central chunk.
        let mut c = Arq {
            center: Loc::from(42),
            power: 2,
            count: 5,
        };
        assert_eq!(
            c.to_interval(),
            ArcInterval::new(40 - 4 * 2, 40 + 4 * 3 - 1)
        );

        // An even chunk count leads to the new chunk being added to the right
        // in this case, since 42 is closer to the right edge of its containing
        // chunk (43) than to the left edge (40)
        c.count = 6;
        assert_eq!(
            c.to_interval(),
            ArcInterval::new(40 - 4 * 2, 40 + 4 * 4 - 1)
        );

        // If the center is shifted by 1, then the opposite is true.
        c.center = Loc::from(*c.center - Wrapping(1));
        assert_eq!(
            c.to_interval(),
            ArcInterval::new(40 - 4 * 3, 40 + 4 * 3 - 1)
        );
    }

    #[test]
    fn coverage_requantize() {
        let c = Arq {
            center: Loc::from(42),
            power: 20,
            count: 10,
        };

        let rq = |a: Arq, p| a.clone().requantize(p).then(|| a);

        assert_eq!(rq(c.clone(), 18).map(|c| c.count), Some(40));
        assert_eq!(rq(c.clone(), 19).map(|c| c.count), Some(20));
        assert_eq!(rq(c.clone(), 20).map(|c| c.count), Some(10));
        assert_eq!(rq(c.clone(), 21).map(|c| c.count), Some(5));
        assert_eq!(rq(c.clone(), 22).map(|c| c.count), None);
        assert_eq!(rq(c.clone(), 23).map(|c| c.count), None);
        assert_eq!(rq(c.clone(), 24).map(|c| c.count), None);

        let c = Arq {
            center: Loc::from(42),
            power: 20,
            count: 256,
        };

        assert_eq!(rq(c.clone(), 12).map(|c| c.count), Some(256 * 256));
        assert_eq!(rq(c.clone(), 28).map(|c| c.count), Some(1));
        assert_eq!(rq(c.clone(), 29).map(|c| c.count), None);
    }
}
