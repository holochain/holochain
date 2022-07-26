use crate::*;
use std::fmt::Write;

/// When sampling a section of the arc we can
/// collect all the other peer [`DhtArc`]s into a
/// DhtBucket.
/// All the peer arcs arc contained within the buckets filter arc.
/// The filter is this peer's "view" into their section of the dht arc.
/// This type is mainly used for Display purposes.
pub struct DhtArcBucket {
    /// The arc used to filter this bucket.
    filter: DhtArc,
    /// The arcs in this bucket.
    arcs: Vec<DhtArc>,
}

impl DhtArcBucket {
    /// Select only the arcs that fit into the bucket.
    pub fn new<I: IntoIterator<Item = DhtArc>>(filter: DhtArc, arcs: I) -> Self {
        let arcs = arcs
            .into_iter()
            .filter(|a| filter.contains(a.start_loc()))
            .collect();
        Self { filter, arcs }
    }

    /// Same as new but doesn't check if arcs fit into the bucket.
    pub fn new_unchecked(bucket: DhtArc, arcs: Vec<DhtArc>) -> Self {
        Self {
            filter: bucket,
            arcs,
        }
    }

    pub fn to_ascii(&self, len: usize) -> String {
        let mut buf = "".to_string();
        for a in &self.arcs {
            buf += &a.to_ascii(len);
        }
        let _ = write!(buf, "{} <- Bucket arc", self.filter.to_ascii(len));
        buf
    }
}
