use crate::*;

/// When sampling a section of the arc we can
/// collect all the other peer [`ArcInterval`]s into a
/// DhtBucket.
/// All the peer arcs arc contained within the buckets filter arc.
/// The filter is this peer's "view" into their section of the dht arc.
/// This type is mainly used for Display purposes.
pub struct DhtArcBucket {
    /// The arc used to filter this bucket.
    filter: ArcInterval,
    /// The arcs in this bucket.
    arcs: Vec<ArcInterval>,
}

impl DhtArcBucket {
    /// Select only the arcs that fit into the bucket.
    pub fn new<I: IntoIterator<Item = ArcInterval>>(filter: ArcInterval, arcs: I) -> Self {
        let arcs = arcs
            .into_iter()
            .filter(|a| filter.contains(a.start_loc()))
            .collect();
        Self { filter, arcs }
    }

    /// Same as new but doesn't check if arcs fit into the bucket.
    pub fn new_unchecked(bucket: ArcInterval, arcs: Vec<ArcInterval>) -> Self {
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
        buf += &format!("{} <- Bucket arc", self.filter.to_ascii(len));
        buf
    }
}
