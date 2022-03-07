use kitsune_p2p_dht_arc::ArcInterval;

use crate::{quantum::Topology, Loc};

use super::{Arq, ArqBounded, ArqBounds};

/// Scale a number in a smaller space (specified by `len`) up into the `u32` space.
/// The number to scale can be negative, which is wrapped to a positive value via modulo
pub(crate) fn loc_upscale(len: usize, v: i32) -> u32 {
    let max = 2f64.powi(32);
    let lenf = len as f64;
    let vf = v as f64;
    (max / lenf * vf) as i64 as u32
}

/// Scale a u32 Loc down into a smaller space (specified by `len`)
pub(crate) fn loc_downscale(len: usize, d: Loc) -> usize {
    let max = 2f64.powi(32);
    let lenf = len as f64;
    ((lenf / max * (d.as_u32() as f64)) as usize) % len
}

impl Arq {
    pub fn to_ascii(&self, topo: &Topology, len: usize) -> String {
        let mut s = self.to_bounds().to_ascii(topo, len);
        if !self.is_empty() {
            let center = loc_downscale(len, self.center);
            s.replace_range(center..center + 1, "@");
        }
        s
    }
}

pub fn add_location_ascii(mut s: String, locs: Vec<Loc>) -> String {
    let len = s.len();

    let mut buf = vec![0; len];
    for loc in locs {
        let loc = loc_downscale(len, loc);
        buf[loc] += 1;
    }
    for (i, v) in buf.into_iter().enumerate() {
        if v > 0 {
            // add hex representation of number of ops in this bucket
            let c = format!("{:x}", v.min(0xf));
            s.replace_range(i..i + 1, &c);
        }
    }
    s
}

impl ArqBounds {
    /// Handy ascii representation of an arc, especially useful when
    /// looking at several arcs at once to get a sense of their overlap
    pub fn to_ascii(&self, topo: &Topology, len: usize) -> String {
        let empty = || " ".repeat(len);
        let full = || "-".repeat(len);

        // If lo and hi are less than one bucket's width apart when scaled down,
        // decide whether to interpret this as empty or full
        let decide = |lo: &Loc, hi: &Loc| {
            let mid = loc_upscale(len, (len / 2) as i32);
            if lo < hi {
                if hi.as_u32() - lo.as_u32() < mid {
                    empty()
                } else {
                    full()
                }
            } else if lo.as_u32() - hi.as_u32() < mid {
                full()
            } else {
                empty()
            }
        };

        match self.to_interval(topo) {
            ArcInterval::Full => full(),
            ArcInterval::Empty => empty(),
            ArcInterval::Bounded(lo0, hi0) => {
                let lo = loc_downscale(len, lo0);
                let hi = loc_downscale(len, hi0);

                if lo0 <= hi0 {
                    if lo >= hi {
                        vec![decide(&lo0, &hi0)]
                    } else {
                        vec![
                            " ".repeat(lo),
                            "-".repeat(hi - lo + 1),
                            " ".repeat((len - hi).saturating_sub(1)),
                        ]
                    }
                } else if lo <= hi {
                    vec![decide(&lo0, &hi0)]
                } else {
                    vec![
                        "-".repeat(hi + 1),
                        " ".repeat((lo - hi).saturating_sub(1)),
                        "-".repeat(len - lo),
                    ]
                }
                .join("")
            }
        }
    }
}
