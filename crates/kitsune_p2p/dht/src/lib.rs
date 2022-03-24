//! Defines the structure of the DHT and the operations which can be performed
//! in and on it.

#![warn(missing_docs)]

pub mod arq;
pub mod error;
pub mod hash;
pub mod op;
pub mod quantum;
pub mod region;
pub mod region_set;

pub use arq::{actual_coverage, Arq, ArqBounds, ArqStrat, PeerStrat, PeerView, PeerViewQ};

// The persistence traits are currently unused except for test implementations of
// a kitsune host. If we ever use them in actual host implementations, we can
// take the feature flag off.
#[cfg(feature = "test_utils")]
pub mod persistence;

#[cfg(feature = "test_utils")]
pub mod test_utils;

pub use kitsune_p2p_dht_arc::DhtLocation as Loc;

/// Common exports
pub mod prelude {
    pub use super::arq::*;
    pub use super::error::*;
    pub use super::hash::*;
    pub use super::op::*;
    pub use super::persistence::*;
    pub use super::quantum::*;
    pub use super::region::*;
    pub use super::region_set::*;
}
