//! KitsuneP2p Wire Protocol Encoding Decoding

use crate::actor::BroadcastTo;
use crate::agent_store::AgentInfoSigned;
use crate::types::*;
use derive_more::*;
use kitsune_p2p_types::dht_arc::DhtLocation;
use std::sync::Arc;

/// Type used for content data of wire messages.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Deref, AsRef, From, Into, serde::Serialize, serde::Deserialize,
)]
pub struct WireData(#[serde(with = "serde_bytes")] pub Vec<u8>);

/// Enum containing the individual metric exchange messages used by clients
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MetricExchangeMsg {
    /// To start off, let's use a naive single message sending
    /// everything we care about.
    V1UniBlast {
        /// The extrapolated coverage calculated by this node
        /// note this is NOT the aggregate the node has collected,
        /// just the direct extrapolation based on known peer infos.
        extrap_cov_f32_le: WireData,
    },

    /// Future proof by having an unknown message catch-all variant
    /// that we can ignore for any future variants that are added
    #[serde(other)]
    UnknownMessage,
}

/// Determine what should be fetched.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FetchKey {
    /// Fetch via region.
    // TODO - add region info
    Region,

    /// Fetch via op hash.
    Op {
        /// The hash of the op to fetch.
        op_hash: Arc<KitsuneBasis>,

        /// If specified, the author of the op.
        /// NOTE: author is addative-only. That is, an op without an author
        /// is the same as one *with* an author, but should be updated to
        /// include the author. It is UB to have two FetchKeys with the
        /// same op_hash, but different authors.
        author: Option<Arc<KitsuneAgent>>,
    },
}

/// A fetch "unit" that can be de-duplicated.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub struct FetchRequest {
    ///
    key: FetchKey,

    /// App-specific optional arguments related to fetching the data.
    _options: Option<WireData>,
}

kitsune_p2p_types::write_codec_enum! {
    /// KitsuneP2p Wire Protocol Top-Level Enum.
    codec Wire {
        /// Failure
        Failure(0x00) {
            reason.0: String,
        },

        /// "Call" to the remote.
        Call(0x10) {
            space.0: Arc<KitsuneSpace>,
            to_agent.1: Arc<KitsuneAgent>,
            data.2: WireData,
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

            destination.5: BroadcastTo,

            data.6: WireData,
        },

        /// Fire-and-forget broadcast message.
        /// uses low-level notify, not request
        Broadcast(0x23) {
            space.0: Arc<KitsuneSpace>,
            to_agent.1: Arc<KitsuneAgent>,
            destination.2: BroadcastTo,
            data.3: WireData,
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
            basis_loc.1: DhtLocation,
        },

        /// Response to a peer query
        PeerQueryResp(0x53) {
            peer_list.0: Vec<AgentInfoSigned>,
        },

        /// Request the peer send op data.
        /// This is sent as a fire-and-forget Notify message.
        /// The "response" is "PushOpData" below.
        FetchOp(0x60) {
            space.0: Arc<KitsuneSpace>,
            fetch_list.1: Vec<FetchRequest>,
        },

        /// This is a fire-and-forget "response" to the
        /// fire-and-forget "FetchOp" request, also sent via Notify.
        PushOpData(0x61) {
            space.0: Arc<KitsuneSpace>,
            op_data_list.1: Vec<WireData>,
        },

        /// MetricsExchangeMessage
        MetricExchange(0xa0) {
            space.0: Arc<KitsuneSpace>,
            msgs.1: Vec<MetricExchangeMsg>,
        },
    }
}
