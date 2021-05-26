//! KitsuneP2p Wire Protocol Encoding Decoding

use crate::agent_store::AgentInfoSigned;
use crate::types::*;
use derive_more::*;
use std::sync::Arc;

/// Type used for content data of wire messages.
#[derive(
    Debug, Clone, PartialEq, Deref, AsRef, From, Into, serde::Serialize, serde::Deserialize,
)]
pub struct WireData(#[serde(with = "serde_bytes")] pub Vec<u8>);

kitsune_p2p_types::write_codec_enum! {
    /// KitsuneP2p Wire Protocol Top-Level Enum.
    codec Wire {
        /// Failure
        Failure(0x00) {
            reason.0: String,
        },

        /// "Call" to the remote.
        Call(0x010) {
            space.0: Arc<KitsuneSpace>,
            from_agent.1: Arc<KitsuneAgent>,
            to_agent.2: Arc<KitsuneAgent>,
            data.3: WireData,
        },

        /// "Call" response from the remote.
        CallResp(0x11) {
            data.0: WireData,
        },

        /// "Notify" the remote.
        Notify(0x20) {
            space.0: Arc<KitsuneSpace>,
            from_agent.1: Arc<KitsuneAgent>,
            to_agent.2: Arc<KitsuneAgent>,
            data.3: WireData,
        },

        /// "Notify" response from the remote.
        NotifyResp(0x21) {
        },

        /// Gossip op with opaque data section,
        /// to be forwarded to gossip module.
        Gossip(0x42) {
            space.0: Arc<KitsuneSpace>,
            data.1: WireData,
        },

        /// Query a remote node for peers holding
        /// or nearest to holding a u32 location.
        PeerQuery(0x50) {
            space.0: Arc<KitsuneSpace>,
            basis.1: Arc<KitsuneBasis>,
        },

        /// Response to a peer query
        PeerQueryResp(0x51) {
            peer_list.0: Vec<AgentInfoSigned>,
        },
    }
}
