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

        /// "DelegateBroadcast" to the remote.
        /// Remote should in turn connect to nodes in neighborhood,
        /// and call "Notify" per broadcast algorithm.
        /// uses low-level notify, not request
        DelegateBroadcast(0x22) {
            space.0: Arc<KitsuneSpace>,
            basis.1: Arc<KitsuneBasis>,
            to_agent.2: Arc<KitsuneAgent>,

            /// If `tgt_agent.get_loc() % mod_cnt == mod_idx`,
            /// we are responsible for broadcasting to tgt_agent.
            mod_idx.3: u32,

            /// see mod_idx description
            mod_cnt.4: u32,

            data.5: WireData,
        },

        /// Fire-and-forget broadcast message.
        /// uses low-level notify, not request
        Broadcast(0x23) {
            space.0: Arc<KitsuneSpace>,
            to_agent.1: Arc<KitsuneAgent>,
            data.2: WireData,
        },

        /// Gossip op with opaque data section,
        /// to be forwarded to gossip module.
        /// uses low-level notify, not request
        Gossip(0x42) {
            space.0: Arc<KitsuneSpace>,
            data.1: WireData,
            module.2: gossip::GossipModuleType,
        },

        /// Ask a remote node if they know about a specific agent
        PeerGet(0x50) {
            space.0: Arc<KitsuneSpace>,
            agent.1: Arc<KitsuneAgent>,
        },

        /// Response to a peer get
        PeerGetResp(0x51) {
            agent_info_signed.0: AgentInfoSigned,
        },

        /// Query a remote node for peers holding
        /// or nearest to holding a u32 location.
        PeerQuery(0x52) {
            space.0: Arc<KitsuneSpace>,
            basis_loc.1: u32,
        },

        /// Response to a peer query
        PeerQueryResp(0x53) {
            peer_list.0: Vec<AgentInfoSigned>,
        },
    }
}
