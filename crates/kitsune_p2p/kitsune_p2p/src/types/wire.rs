//! KitsuneP2p Wire Protocol Encoding Decoding

use crate::agent_store::AgentInfoSigned;
use crate::types::gossip::{OpConsistency, OpCount};
use crate::types::*;
use derive_more::*;
use kitsune_p2p_types::dht_arc::DhtArc;
use once_cell::sync::Lazy;
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

        /// Fetch DhtOp and Agent Hashes with Constraints
        FetchOpHashes(0x31) {
            space.0: Arc<KitsuneSpace>,
            from_agent.1: Arc<KitsuneAgent>,
            to_agent.2: Arc<KitsuneAgent>,
            dht_arc.3: DhtArc,
            since_utc_epoch_s.4: i64,
            until_utc_epoch_s.5: i64,
            last_count.6: OpCount,
        },

        /// List of hashes response to FetchOpHashes
        FetchOpHashesResponse(0x32) {
            hashes.0: OpConsistency,
            peer_hashes.1: Vec<(Arc<KitsuneAgent>, u64)>,
        },

        /// Fetch DhtOp data and AgentInfo for hashes lists
        FetchOpData(0x33) {
            space.0: Arc<KitsuneSpace>,
            from_agent.1: Arc<KitsuneAgent>,
            to_agent.2: Arc<KitsuneAgent>,
            op_hashes.3: Vec<Arc<KitsuneOpHash>>,
            peer_hashes.4: Vec<Arc<KitsuneAgent>>,
        },

        /// Lists of data in response to FetchOpData
        FetchOpDataResponse(0x34) {
            op_data.0: Vec<(Arc<KitsuneOpHash>, WireData)>,
            agent_infos.1: Vec<AgentInfoSigned>,
        },

        /// Query Agent data from a remote node
        AgentInfoQuery(0x40) {
            space.0: Arc<KitsuneSpace>,
            to_agent.1: Arc<KitsuneAgent>,
            by_agent.2: Option<Arc<KitsuneAgent>>,
            by_basis_arc.3: Option<(Arc<KitsuneBasis>, DhtArc)>,
        },

        /// Response type for agent info query
        AgentInfoQueryResp(0x41) {
            agent_infos.0: Vec<AgentInfoSigned>,
        },

        /// Fetch DhtOp data and AgentInfo for hashes lists
        Gossip(0x50) {
            space.0: Arc<KitsuneSpace>,
            from_agent.1: Arc<KitsuneAgent>,
            to_agent.2: Arc<KitsuneAgent>,
            ops.3: Vec<(Arc<KitsuneOpHash>, WireData)>,
            agents.4: Vec<AgentInfoSigned>,
        },

        /// Lists of data in response to FetchOpData
        GossipResp(0x51) {
        },
    }
}

pub(crate) type DiscriminantMap =
    std::collections::HashMap<std::mem::Discriminant<wire::Wire>, crate::metrics::KitsuneMetrics>;

/// This is used to map wire types to the KitsuneMetrics enum type.
/// Unfortunately, the only way to get a discriminant for a variant
/// is to have an actual variant instance. This lazy represents
/// a first-access one-time cost of creating dummy variants
/// to generate the discriminants.
pub(crate) static DISC_MAP: Lazy<DiscriminantMap> = Lazy::new(|| {
    use crate::metrics::KitsuneMetrics;
    let mut map = std::collections::HashMap::new();

    let space = Arc::new(KitsuneSpace(vec![]));
    let agent = Arc::new(KitsuneAgent(vec![]));

    map.insert(
        std::mem::discriminant(&Wire::Failure(Failure {
            reason: "".to_string(),
        })),
        KitsuneMetrics::Fail,
    );
    map.insert(
        std::mem::discriminant(&Wire::Call(Call {
            space: space.clone(),
            from_agent: agent.clone(),
            to_agent: agent.clone(),
            data: WireData(vec![]),
        })),
        KitsuneMetrics::Call,
    );
    map.insert(
        std::mem::discriminant(&Wire::CallResp(CallResp {
            data: WireData(vec![]),
        })),
        KitsuneMetrics::CallResp,
    );
    map.insert(
        std::mem::discriminant(&Wire::Notify(Notify {
            space: space.clone(),
            from_agent: agent.clone(),
            to_agent: agent.clone(),
            data: WireData(vec![]),
        })),
        KitsuneMetrics::Call,
    );
    map.insert(
        std::mem::discriminant(&Wire::NotifyResp(NotifyResp {})),
        KitsuneMetrics::NotifyResp,
    );
    map.insert(
        std::mem::discriminant(&Wire::FetchOpHashes(FetchOpHashes {
            space: space.clone(),
            from_agent: agent.clone(),
            to_agent: agent.clone(),
            dht_arc: DhtArc {
                center_loc: 0.into(),
                half_length: 0,
            },
            since_utc_epoch_s: 0,
            until_utc_epoch_s: 0,
            last_count: OpCount::Variance,
        })),
        KitsuneMetrics::FetchOpHashes,
    );
    map.insert(
        std::mem::discriminant(&Wire::FetchOpHashesResponse(FetchOpHashesResponse {
            hashes: OpConsistency::Consistent,
            peer_hashes: vec![],
        })),
        KitsuneMetrics::FetchOpHashesResp,
    );
    map.insert(
        std::mem::discriminant(&Wire::FetchOpData(FetchOpData {
            space: space.clone(),
            from_agent: agent.clone(),
            to_agent: agent.clone(),
            op_hashes: vec![],
            peer_hashes: vec![],
        })),
        KitsuneMetrics::FetchOpData,
    );
    map.insert(
        std::mem::discriminant(&Wire::FetchOpDataResponse(FetchOpDataResponse {
            op_data: vec![],
            agent_infos: vec![],
        })),
        KitsuneMetrics::FetchOpDataResp,
    );
    map.insert(
        std::mem::discriminant(&Wire::AgentInfoQuery(AgentInfoQuery {
            space: space.clone(),
            to_agent: agent.clone(),
            by_agent: None,
            by_basis_arc: None,
        })),
        KitsuneMetrics::AgentInfoQuery,
    );
    map.insert(
        std::mem::discriminant(&Wire::AgentInfoQueryResp(AgentInfoQueryResp {
            agent_infos: vec![],
        })),
        KitsuneMetrics::AgentInfoQuery,
    );
    map.insert(
        std::mem::discriminant(&Wire::Gossip(Gossip {
            space,
            from_agent: agent.clone(),
            to_agent: agent,
            ops: vec![],
            agents: vec![],
        })),
        KitsuneMetrics::Gossip,
    );
    map.insert(
        std::mem::discriminant(&Wire::GossipResp(GossipResp {})),
        KitsuneMetrics::GossipResp,
    );
    map
});
