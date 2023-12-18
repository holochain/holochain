//! # DNA Properties Support types

use holo_hash::DnaHashB64;
use holochain_serialized_bytes::prelude::*;

/// Extra parameters that contribute to determining the DNA hash.
/// They are set by the conductor at install time and cannot be specified
/// by the DNA developer.
/// They represent different aspects of networking compability.
/// Two conductors using different networking protocols or two different
/// DPKI services will not be able to communicate over the network and are effectively
/// in their own separate networks. By including these parameters in the DNA hash,
/// we make this compatibility explicit, so that two cells will be able to communicate
/// over the same network if and only if their DNA hashes are the same.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct DnaNetworkParams {
    /// A version number which represents network protocol compatibility.
    /// This is set by kitsune and bumped whenever a breaking protocol change is made.
    pub protocol_version: u32,

    /// DPKI is fundamental to the workings of agent key management and validation.
    /// Two conductors with different DPKI networks cannot validate each other's agent keys,
    /// effectively cutting them off from each other, so we treat this as a determinant
    /// of network compatibility.
    ///
    /// Note that conductors with no DPKI service installed will be able to talk to conductors
    /// with a DPKI service installed on the same network, but not vice versa,
    /// so we still ensure that both cases result in a different DNA hash so that we don't have
    /// to consider that kind of one-way communication.
    pub dpki_hash: Option<DnaHashB64>,
}

#[cfg(feature = "test_utils")]
impl DnaNetworkParams {
    /// Get a fake value for testing
    pub fn fake() -> Self {
        DnaNetworkParams {
            protocol_version: 0xFA73, // 64115
            dpki_hash: None,
        }
    }
}
