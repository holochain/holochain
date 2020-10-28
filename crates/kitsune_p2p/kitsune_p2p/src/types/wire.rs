//! KitsuneP2p Wire Protocol Encoding Decoding

use crate::agent_store::AgentInfoSigned;
use derive_more::*;
use kitsune_p2p_types::dht_arc::DhtArc;

/// Type used for content data of wire messages.
#[derive(Debug, PartialEq, Deref, AsRef, From, Into, serde::Serialize, serde::Deserialize)]
pub struct WireData(#[serde(with = "serde_bytes")] pub Vec<u8>);

kitsune_p2p_types::write_codec_enum! {
    /// KitsuneP2p Wire Protocol Top-Level Enum.
    codec Wire {
        /// "Call" to the remote.
        Call(0x01) {
            data.0: WireData,
        },

        /// "Notify" the remote.
        Notify(0x02) {
            data.0: WireData,
        },

        /// Publish Signed Agent Info
        AgentPublish(0x10) {
            data.0: AgentInfoSigned,
        },

        /// Fetch Agent Info Hashes with Constraints
        AgentFetchHashes(0x11) {
            dht_arc.0: DhtArc,
            since_utc_epoch_s.1: i64,
            until_utc_epoch_s.2: i64,
            hashes.3: Vec<WireData>,
        },

        /// List of hashes response to AgentFetchHashes
        AgentFetchHashesResponse(0x12) {
            hashes.0: Vec<WireData>,
        },

        /// Fetch Agent Info Data for Hash List
        AgentFetchDataForHashList(0x13) {
            hashes.0: Vec<WireData>,
        },

        /// List of agent data response to AgentFetchDataForHashList
        AgentFetchDataForHashListResponse(0x14) {
            agent_info.0: Vec<(WireData, AgentInfoSigned)>,
        },
    }
}
