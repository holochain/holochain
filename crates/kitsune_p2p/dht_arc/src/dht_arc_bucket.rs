use crate::*;

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
            .filter(|a| filter.contains(a.center_loc))
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
}

impl std::fmt::Display for DhtArcBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for a in &self.arcs {
            writeln!(f, "{}", a)?;
        }
        writeln!(f, "{} <- Bucket arc", self.filter)
    }
}
