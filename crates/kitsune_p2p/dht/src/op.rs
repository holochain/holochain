//! Defines the trait which represents everything Kitsune needs to know about Ops

use crate::quantum::{SpacetimeQuantumCoords, Topology};

pub use kitsune_p2p_dht_arc::DhtLocation as Loc;

pub use kitsune_p2p_timestamp::Timestamp;

/// Everything that Kitsune needs to know about an Op.
/// Intended to be implemented by the host.
pub trait OpRegion<D>: PartialOrd + Ord + Send + Sync {
    /// The op's Location
    fn loc(&self) -> Loc;
    /// The op's Timestamp
    fn timestamp(&self) -> Timestamp;
    /// The RegionData that would be produced if this op were the only op
    /// in the region. The sum of these produces the RegionData for the whole
    /// region.
    fn region_data(&self) -> D;

    /// The quantized space and time coordinates, based on the location and timestamp.
    fn coords(&self, topo: &Topology) -> SpacetimeQuantumCoords {
        SpacetimeQuantumCoords {
            space: topo.space_coord(self.loc()),
            time: topo.time_coord(self.timestamp()),
        }
    }

    /// Create an Op with arbitrary data but that has the given timestamp and location.
    /// Used for bounded range queries based on the PartialOrd impl of the op.
    fn bound(timestamp: Timestamp, loc: Loc) -> Self;
}
