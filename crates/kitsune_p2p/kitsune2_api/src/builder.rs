//! Builder-related types.

use crate::*;

/// The general Kitsune2 builder.
/// Contains the factory types that will be used for constructing modules.
/// TODO - this should also contain configuration TBD.
#[derive(Clone)]
pub struct Builder {
    /// The [PeerStoreFactory](peer_store::PeerStoreFactory) to be used for
    /// creating [peer_store::PeerStore] instances.
    pub peer_store: peer_store::DynPeerStoreFactory,
}
