//! Definitions for events emited from the KitsuneP2p actor.

use std::sync::Arc;

/// Gather a list of op-hashes from our implementor that meet criteria.
pub struct FetchOpHashesForConstraintsEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
    /// The start point on the dht arc to query.
    pub dht_arc_start_loc: u32,
    /// The arc-length to query.
    pub dht_arc_length: u64,
    /// If specified, only retreive items received since this time.
    pub since: Option<std::time::SystemTime>,
    /// If specified, only retreive items received until this time.
    pub until: Option<std::time::SystemTime>,
}

/// Gather all op-hash data for a list of op-hashes from our implementor.
pub struct FetchOpHashDataEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
    /// The op-hashes to fetch
    pub op_hashes: Arc<Vec<Arc<super::KitsuneOpHash>>>,
}

/// Request that our implementor sign some data on behalf of an agent.
pub struct SignNetworkDataEvt {
    /// The "space" context.
    pub space: Arc<super::KitsuneSpace>,
    /// The "agent" context.
    pub agent: Arc<super::KitsuneAgent>,
    /// The data to sign.
    pub data: Arc<Vec<u8>>,
}

ghost_actor::ghost_actor! {
    /// The KitsuneP2pEvent stream allows handling events generated from the
    /// KitsuneP2p actor.
    pub actor KitsuneP2pEvent<super::KitsuneP2pError> {
        /// We are receiving a request from a remote node.
        fn call(space: Arc<super::KitsuneSpace>, agent: Arc<super::KitsuneAgent>, payload: Vec<u8>) -> Vec<u8>;

        /// We are receiving a notification from a remote node.
        fn notify(space: Arc<super::KitsuneSpace>, agent: Arc<super::KitsuneAgent>, payload: Vec<u8>) -> ();

        /// Gather a list of op-hashes from our implementor that meet criteria.
        fn fetch_op_hashes_for_constraints(input: FetchOpHashesForConstraintsEvt) -> Vec<(super::KitsuneDataHash, Vec<super::KitsuneOpHash>)>;

        /// Gather all op-hash data for a list of op-hashes from our implementor.
        fn fetch_op_hash_data(input: FetchOpHashDataEvt) -> Vec<(super::KitsuneOpHash, Vec<u8>)>;

        /// Request that our implementor sign some data on behalf of an agent.
        fn sign_network_data(input: SignNetworkDataEvt) -> super::KitsuneSignature;
    }
}

/// Receiver type for incoming connection events.
pub type KitsuneP2pEventReceiver = futures::channel::mpsc::Receiver<KitsuneP2pEvent>;
