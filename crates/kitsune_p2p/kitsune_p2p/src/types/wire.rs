//! KitsuneP2p Wire Protocol Encoding Decoding

use crate::{agent_store::AgentInfoSigned, types::*};
use derive_more::*;
use kitsune_p2p_types::dht_arc::DhtArc;
use std::sync::Arc;

/// Type used for content data of wire messages.
#[derive(Debug, PartialEq, Deref, AsRef, From, Into, serde::Serialize, serde::Deserialize)]
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

        /// Publish Signed Agent Info
        AgentPublish(0x30) {
            data.0: AgentInfoSigned,
        },

        /// Fetch Agent Info Hashes with Constraints
        AgentFetchHashes(0x31) {
            dht_arc.0: DhtArc,
            since_utc_epoch_s.1: i64,
            until_utc_epoch_s.2: i64,
            hashes.3: Vec<WireData>,
        },

        /// List of hashes response to AgentFetchHashes
        AgentFetchHashesResponse(0x32) {
            hashes.0: Vec<WireData>,
        },

        /// Fetch Agent Info Data for Hash List
        AgentFetchDataForHashList(0x33) {
            hashes.0: Vec<WireData>,
        },

        /// List of agent data response to AgentFetchDataForHashList
        AgentFetchDataForHashListResponse(0x34) {
            agent_info.0: Vec<(WireData, AgentInfoSigned)>,
        },
    }
}
