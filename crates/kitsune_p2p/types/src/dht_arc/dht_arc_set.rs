use super::DhtArc;

/// A compact representation of a set of DhtArcs, optimized for quickly
/// computing intersections and unions with other DhtArcSets
pub struct DhtArcSet;

impl DhtArcSet {
    /// Cheaply check if the two arcsets have a non-null intersection
    pub fn intersects(&self, other: &Self) -> bool {
        todo!()
    }

    /// Compute the full intersection of the two arcsets
    pub fn intersection(&self, other: &Self) -> Self {
        todo!()
    }

    /// Compute the full union of the two arcsets
    pub fn union(&self, other: &Self) -> Self {
        todo!()
    }
}

/// An alternate implementation of `ArcRange`
#[derive(Clone, Debug)]
pub struct ArcInterval;

impl ArcInterval {
    /// Constructor
    pub fn new(start: u32, end: u32) -> Self {
        todo!()
    }

    /// Constructor
    pub fn new_empty() -> Self {
        todo!()
    }
}

impl From<ArcInterval> for DhtArcSet {
    fn from(_: ArcInterval) -> Self {
        todo!()
    }
}

impl From<DhtArc> for DhtArcSet {
    fn from(_: DhtArc) -> Self {
        todo!()
    }
}
