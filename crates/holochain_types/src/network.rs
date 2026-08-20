//! Types for interacting with Holochain's network layer.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use holo_hash::DnaHash;
use holochain_timestamp::Timestamp;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

/// Encode a Kitsune2 [`kitsune2_api::Id`]-based identifier (`AgentId`, `SpaceId`,
/// `OpId`) as the base64url-no-pad string that Kitsune2 puts on the wire.
///
/// Reads the id's raw bytes directly rather than going through
/// `Display`/`ToString`: `holochain_p2p` installs a process-wide `Display`
/// override on these types (see
/// `kitsune2_api::AgentId::set_global_display_callback`) so logs show
/// Holochain's own hash format, which no longer matches Kitsune2's wire
/// encoding once installed.
pub fn kitsune_id_to_base64url(id: &kitsune2_api::Id) -> String {
    URL_SAFE_NO_PAD.encode(&id.0)
}

/// Request network metrics from Kitsune2.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Kitsune2NetworkMetricsRequest {
    /// Request metrics for a specific DNA.
    ///
    /// If this is blank, then metrics for all DNAs will be returned.
    pub dna_hash: Option<DnaHash>,

    /// Include DHT summary in the response.
    pub include_dht_summary: bool,
}

/// Network metrics from Kitsune2.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct Kitsune2NetworkMetrics {
    /// A summary of the fetch queue.
    ///
    /// The fetch queue is used to retrieve op data based on op ids that have been discovered
    /// through publish or gossip.
    pub fetch_state_summary: FetchStateSummary,
    /// A summary of the gossip state.
    ///
    /// This includes both live gossip rounds and metrics about peers that we've gossiped with.
    /// Optionally, it can include a summary of the DHT state as Kitsune2 sees it.
    pub gossip_state_summary: GossipStateSummary,

    /// A summary of the state of each local agent.
    pub local_agents: Vec<LocalAgentSummary>,
}

/// Summary of a local agent's network state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct LocalAgentSummary {
    /// The agent's public key.
    pub agent: holo_hash::AgentPubKey,

    /// The current storage arc that the agent is declaring.
    ///
    /// This is the arc that the agent is claiming that it is an authority for.
    pub storage_arc: DhtArc,

    /// The target arc that the agent is trying to achieve as a storage arc.
    ///
    /// This is not declared to other peers on the network. It is used during gossip to try to sync
    /// ops in the target arc. Once the DHT state appears to be in sync with the target arc, the
    /// storage arc can be updated towards the target arc.
    pub target_arc: DhtArc,
}

/// Similar struct to [`ApiTransportStats`](kitsune2_api::ApiTransportStats) but with Holochain types.
///
/// There is a [`DnaHash`] instead of a [`Space`](kitsune2_api::Space) in the `blocked_message_counts` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct HolochainTransportStats {
    /// Stats for a transport connection.
    pub transport_stats: TransportStats,

    /// Blocked message counts.
    // ts-rs renders `HashMap<K, V>` as `{ [key in K]?: V }`, invalid unless
    // `K` is a primitive — `DnaHash` isn't. The `string` key reflects
    // holochain-client-js's `holoHashMapKeyConverter`, which rewrites the
    // wire's binary keys into base64 on decode. Routed through the
    // `BlockedMessageCountsMapTs` alias (not a bare `ts(type = "...")`
    // string) so `MessageBlockCount`'s export is guaranteed via a real
    // dependency, not incidental to some other type exporting it.
    #[cfg_attr(feature = "ts_rs", ts(as = "BlockedMessageCountsMapTs"))]
    pub blocked_message_counts: HashMap<String, HashMap<DnaHash, MessageBlockCount>>,
}

/// Mirror of `kitsune2_api::DhtArc` for the conductor API.
///
/// Serializes untagged, exactly like the kitsune2 original:
/// `Empty` maps to null, `Arc(a, b)` maps to `[a, b]`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(untagged)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub enum DhtArc {
    /// No DHT locations are contained within this arc.
    #[default]
    Empty,
    /// A specific range of DHT locations are contained within this arc.
    ///
    /// The lower and upper bounds are inclusive.
    Arc(u32, u32),
}

impl From<kitsune2_api::DhtArc> for DhtArc {
    fn from(a: kitsune2_api::DhtArc) -> Self {
        match a {
            kitsune2_api::DhtArc::Empty => Self::Empty,
            kitsune2_api::DhtArc::Arc(lo, hi) => Self::Arc(lo, hi),
        }
    }
}

/// Mirror of `kitsune2_api::FetchStateSummary`.
///
/// Op ids and peer urls are their wire forms: base64url-no-pad strings and
/// url strings respectively.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct FetchStateSummary {
    /// The op ids that are currently being fetched, each mapped to the peer urls
    /// they could be requested from.
    pub pending_requests: HashMap<String, Vec<String>>,
}

impl From<kitsune2_api::FetchStateSummary> for FetchStateSummary {
    fn from(s: kitsune2_api::FetchStateSummary) -> Self {
        Self {
            pending_requests: s
                .pending_requests
                .into_iter()
                .map(|(op_id, urls)| {
                    let urls = urls.into_iter().map(|u| u.as_str().to_string()).collect();
                    (kitsune_id_to_base64url(&op_id), urls)
                })
                .collect(),
        }
    }
}

/// Mirror of `kitsune2_api::GossipRoundStateSummary`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct GossipRoundStateSummary {
    /// The URL of the peer with which the round is initiated.
    pub session_with_peer: String,
}

impl From<kitsune2_api::GossipRoundStateSummary> for GossipRoundStateSummary {
    fn from(s: kitsune2_api::GossipRoundStateSummary) -> Self {
        Self {
            session_with_peer: s.session_with_peer.as_str().to_string(),
        }
    }
}

/// Mirror of `kitsune2_api::DhtSegmentState`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct DhtSegmentState {
    /// The top hash of the DHT ring segment.
    #[serde(with = "serde_bytes")]
    #[cfg_attr(feature = "ts_rs", ts(type = "Uint8Array"))]
    pub disc_top_hash: Vec<u8>,
    /// The boundary timestamp of the DHT ring segment.
    pub disc_boundary: Timestamp,
    /// The top hashes of each DHT ring segment.
    #[cfg_attr(feature = "ts_rs", ts(type = "Uint8Array[]"))]
    pub ring_top_hashes: Vec<serde_bytes::ByteBuf>,
}

impl From<kitsune2_api::DhtSegmentState> for DhtSegmentState {
    fn from(s: kitsune2_api::DhtSegmentState) -> Self {
        Self {
            disc_top_hash: s.disc_top_hash.to_vec(),
            disc_boundary: Timestamp::from_micros(s.disc_boundary.as_micros()),
            ring_top_hashes: s
                .ring_top_hashes
                .into_iter()
                .map(|h| serde_bytes::ByteBuf::from(h.to_vec()))
                .collect(),
        }
    }
}

/// Mirror of `kitsune2_api::PeerMeta`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct PeerMeta {
    /// The timestamp of the last gossip round.
    pub last_gossip_timestamp: Option<Timestamp>,
    /// The bookmark of the last op bookmark received.
    pub new_ops_bookmark: Option<Timestamp>,
    /// The number of behavior errors observed.
    pub peer_behavior_errors: Option<u32>,
    /// The number of local errors.
    pub local_errors: Option<u32>,
    /// The number of busy peer errors.
    pub peer_busy: Option<u32>,
    /// The number of terminated rounds.
    ///
    /// Note that termination is not necessarily an error.
    pub peer_terminated: Option<u32>,
    /// The number of completed rounds.
    pub completed_rounds: Option<u32>,
    /// The number of peer timeouts.
    pub peer_timeouts: Option<u32>,
    /// The total DHT op count reported by this peer.
    pub dht_op_count: Option<u64>,
    /// Whether this peer has declared itself as offline, and no longer reachable, with a tombstone.
    pub is_tombstone: bool,
    /// The storage arc that this peer is declaring.
    pub storage_arc: DhtArc,
}

impl From<kitsune2_api::PeerMeta> for PeerMeta {
    fn from(p: kitsune2_api::PeerMeta) -> Self {
        Self {
            last_gossip_timestamp: p
                .last_gossip_timestamp
                .map(|t| Timestamp::from_micros(t.as_micros())),
            new_ops_bookmark: p
                .new_ops_bookmark
                .map(|t| Timestamp::from_micros(t.as_micros())),
            peer_behavior_errors: p.peer_behavior_errors,
            local_errors: p.local_errors,
            peer_busy: p.peer_busy,
            peer_terminated: p.peer_terminated,
            completed_rounds: p.completed_rounds,
            peer_timeouts: p.peer_timeouts,
            dht_op_count: p.dht_op_count,
            is_tombstone: p.is_tombstone,
            storage_arc: p.storage_arc.into(),
        }
    }
}

/// Mirror of `kitsune2_api::GossipStateSummary`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct GossipStateSummary {
    /// The current initiated round summary.
    pub initiated_round: Option<GossipRoundStateSummary>,
    /// The list of accepted round summaries.
    pub accepted_rounds: Vec<GossipRoundStateSummary>,
    /// DHT summary.
    pub dht_summary: HashMap<String, DhtSegmentState>,
    /// Peer metadata dump for each agent in this space, keyed by peer url.
    pub peer_meta: HashMap<String, PeerMeta>,
    /// An estimate of the local node's op count.
    pub local_op_count: u64,
}

impl From<kitsune2_api::GossipStateSummary> for GossipStateSummary {
    fn from(s: kitsune2_api::GossipStateSummary) -> Self {
        Self {
            initiated_round: s.initiated_round.map(Into::into),
            accepted_rounds: s.accepted_rounds.into_iter().map(Into::into).collect(),
            dht_summary: s
                .dht_summary
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            peer_meta: s
                .peer_meta
                .into_iter()
                .map(|(url, meta)| (url.as_str().to_string(), meta.into()))
                .collect(),
            local_op_count: s.local_op_count,
        }
    }
}

/// Mirror of `kitsune2_api::TransportConnectionStats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct TransportConnectionStats {
    /// The public key of the remote peer.
    pub pub_key: String,
    /// The message count sent on this connection.
    pub send_message_count: u64,
    /// The bytes sent on this connection.
    pub send_bytes: u64,
    /// The message count received on this connection.
    pub recv_message_count: u64,
    /// The bytes received on this connection.
    pub recv_bytes: u64,
    /// UNIX epoch timestamp in seconds when this connection was opened.
    pub opened_at_s: u64,
    /// True if this connection has successfully upgraded to a direct peer connection.
    pub is_direct: bool,
}

impl From<kitsune2_api::TransportConnectionStats> for TransportConnectionStats {
    fn from(s: kitsune2_api::TransportConnectionStats) -> Self {
        Self {
            pub_key: s.pub_key,
            send_message_count: s.send_message_count,
            send_bytes: s.send_bytes,
            recv_message_count: s.recv_message_count,
            recv_bytes: s.recv_bytes,
            opened_at_s: s.opened_at_s,
            is_direct: s.is_direct,
        }
    }
}

/// Mirror of `kitsune2_api::TransportStats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct TransportStats {
    /// The networking backend that is in use.
    pub backend: String,
    /// The list of peer urls that this Kitsune2 instance can currently be reached at.
    pub peer_urls: Vec<String>,
    /// The list of current connections.
    pub connections: Vec<TransportConnectionStats>,
}

impl From<kitsune2_api::TransportStats> for TransportStats {
    fn from(s: kitsune2_api::TransportStats) -> Self {
        Self {
            backend: s.backend,
            peer_urls: s
                .peer_urls
                .into_iter()
                .map(|u| u.as_str().to_string())
                .collect(),
            connections: s.connections.into_iter().map(Into::into).collect(),
        }
    }
}

/// Mirror of `kitsune2_api::MessageBlockCount`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct MessageBlockCount {
    /// Count of incoming messages that have been blocked and dropped.
    pub incoming: u32,
    /// Count of outgoing messages that have been blocked and dropped.
    pub outgoing: u32,
}

impl From<kitsune2_api::MessageBlockCount> for MessageBlockCount {
    fn from(c: kitsune2_api::MessageBlockCount) -> Self {
        Self {
            incoming: c.incoming,
            outgoing: c.outgoing,
        }
    }
}

// See `HolochainTransportStats.blocked_message_counts`'s doc comment for why
// this is a named `ts_alias!` rather than an inline `ts(type = "...")`
// string override.
#[cfg(feature = "ts_rs")]
holo_hash::ts_alias!(
    BlockedMessageCountsMapTs,
    "BlockedMessageCountsMap",
    "Record<string, Record<string, MessageBlockCount>>",
    "api/admin/types.ts",
    deps: [MessageBlockCount]
);

#[cfg(test)]
mod wire_compat {
    use super::*;

    fn round_trip<K: serde::Serialize, M: serde::de::DeserializeOwned + serde::Serialize>(k: &K) {
        let wire = rmp_serde::to_vec_named(k).unwrap();
        let mirror: M = rmp_serde::from_slice(&wire).unwrap();
        assert_eq!(rmp_serde::to_vec_named(&mirror).unwrap(), wire);
    }

    #[test]
    fn dht_arc_wire_compat() {
        round_trip::<_, DhtArc>(&kitsune2_api::DhtArc::Empty);
        round_trip::<_, DhtArc>(&kitsune2_api::DhtArc::Arc(0, u32::MAX));
    }

    #[test]
    fn fetch_state_summary_wire_compat() {
        let mut pending_requests = HashMap::new();
        pending_requests.insert(
            kitsune2_api::OpId::from(bytes::Bytes::from_static(b"op-1")),
            vec![kitsune2_api::Url::from_str("wss://test.com:443").unwrap()],
        );

        round_trip::<_, FetchStateSummary>(&kitsune2_api::FetchStateSummary { pending_requests });
    }

    #[test]
    fn gossip_state_summary_wire_compat() {
        let peer_url = kitsune2_api::Url::from_str("wss://test.com:443").unwrap();

        let mut dht_summary = HashMap::new();
        dht_summary.insert(
            "segment-0".to_string(),
            kitsune2_api::DhtSegmentState {
                disc_top_hash: bytes::Bytes::from_static(b"top-hash"),
                disc_boundary: kitsune2_api::Timestamp::from_micros(42),
                ring_top_hashes: vec![bytes::Bytes::from_static(b"ring-hash")],
            },
        );

        let mut peer_meta = HashMap::new();
        peer_meta.insert(
            peer_url.clone(),
            kitsune2_api::PeerMeta {
                last_gossip_timestamp: Some(kitsune2_api::Timestamp::from_micros(1)),
                new_ops_bookmark: Some(kitsune2_api::Timestamp::from_micros(2)),
                peer_behavior_errors: Some(1),
                local_errors: Some(2),
                peer_busy: Some(3),
                peer_terminated: Some(4),
                completed_rounds: Some(5),
                peer_timeouts: Some(6),
                dht_op_count: Some(7),
                is_tombstone: false,
                storage_arc: kitsune2_api::DhtArc::Arc(0, u32::MAX),
            },
        );

        let summary = kitsune2_api::GossipStateSummary {
            initiated_round: Some(kitsune2_api::GossipRoundStateSummary {
                session_with_peer: peer_url.clone(),
            }),
            accepted_rounds: vec![kitsune2_api::GossipRoundStateSummary {
                session_with_peer: peer_url,
            }],
            dht_summary,
            peer_meta,
            local_op_count: 99,
        };

        round_trip::<_, GossipStateSummary>(&summary);
    }

    #[test]
    fn transport_stats_wire_compat() {
        let stats = kitsune2_api::TransportStats {
            backend: "test-backend".to_string(),
            peer_urls: vec![kitsune2_api::Url::from_str("wss://test.com:443").unwrap()],
            connections: vec![kitsune2_api::TransportConnectionStats {
                pub_key: "pub-key".to_string(),
                send_message_count: 1,
                send_bytes: 2,
                recv_message_count: 3,
                recv_bytes: 4,
                opened_at_s: 5,
                is_direct: true,
            }],
        };

        round_trip::<_, TransportStats>(&stats);
    }

    #[test]
    fn message_block_count_wire_compat() {
        round_trip::<_, MessageBlockCount>(&kitsune2_api::MessageBlockCount {
            incoming: 1,
            outgoing: 2,
        });
    }

    #[test]
    fn kitsune_id_to_base64url_matches_kitsune2_serialize() {
        let raw = bytes::Bytes::from_static(b"kitsune-id-raw-bytes-fixture");
        let op_id = kitsune2_api::OpId::from(raw.clone());
        let agent_id = kitsune2_api::AgentId::from(raw.clone());
        let space_id = kitsune2_api::SpaceId::from(raw.clone());

        let expected = URL_SAFE_NO_PAD.encode(&raw);

        // The conversion function must match the raw base64url-no-pad encoding
        // of the id's bytes for every id type.
        assert_eq!(kitsune_id_to_base64url(&op_id), expected);
        assert_eq!(kitsune_id_to_base64url(&agent_id), expected);
        assert_eq!(kitsune_id_to_base64url(&space_id), expected);

        // ...and it must match exactly what Kitsune2's own `Serialize` impl
        // puts on the wire for each id type, extracted by round-tripping
        // through the same encoding the conductor API uses.
        let op_id_wire: String =
            rmp_serde::from_slice(&rmp_serde::to_vec_named(&op_id).unwrap()).unwrap();
        let agent_id_wire: String =
            rmp_serde::from_slice(&rmp_serde::to_vec_named(&agent_id).unwrap()).unwrap();
        let space_id_wire: String =
            rmp_serde::from_slice(&rmp_serde::to_vec_named(&space_id).unwrap()).unwrap();

        assert_eq!(op_id_wire, expected);
        assert_eq!(agent_id_wire, expected);
        assert_eq!(space_id_wire, expected);
    }

    #[test]
    fn kitsune_id_to_base64url_ignores_display_override() {
        // Simulate what `holochain_p2p::check_k2_init` installs process-wide in
        // every real conductor: an `OpId` `Display` override that renders
        // something other than the Kitsune2 wire encoding.
        // `kitsune_id_to_base64url` must read the id's raw bytes directly and
        // stay correct regardless of this override.
        // The override is process-wide and can only be installed before the
        // first `OpId` display, so another test in this process may have
        // claimed it first. The wire encoding must hold either way; only the
        // check that the override took effect depends on winning that race.
        let override_installed = kitsune2_api::OpId::set_global_display_callback(|_bytes, f| {
            f.write_str("not-the-wire-value")
        });

        let raw = bytes::Bytes::from_static(b"op-id-under-display-override");
        let op_id = kitsune2_api::OpId::from(raw.clone());

        if override_installed {
            assert_eq!(op_id.to_string(), "not-the-wire-value");
        }

        assert_eq!(
            kitsune_id_to_base64url(&op_id),
            URL_SAFE_NO_PAD.encode(&raw)
        );
    }

    #[test]
    fn fetch_state_summary_from_uses_wire_bytes_for_op_id_key() {
        let raw = bytes::Bytes::from_static(b"fetch-state-summary-op-id");
        let op_id = kitsune2_api::OpId::from(raw.clone());
        let url = kitsune2_api::Url::from_str("wss://test.com:443").unwrap();

        let mut pending_requests = HashMap::new();
        pending_requests.insert(op_id, vec![url.clone()]);

        // Exercise the real production conversion, not just structural
        // round-trip compatibility.
        let mirror: FetchStateSummary = kitsune2_api::FetchStateSummary { pending_requests }.into();

        let expected_key = URL_SAFE_NO_PAD.encode(&raw);
        assert_eq!(
            mirror.pending_requests.get(&expected_key),
            Some(&vec![url.as_str().to_string()])
        );
    }
}

#[cfg(all(test, feature = "ts_rs"))]
mod ts_export {
    use super::*;
    use ts_rs::TS;

    #[test]
    fn dht_arc_untagged_shape_is_null_or_pair() {
        // `DhtArc` is `#[serde(untagged)]`: `Empty` -> null, `Arc(a, b)` -> [a,
        // b]. Assert ts-rs's serde-compat produces that exact shape rather
        // than assuming it handles untagged enums correctly.
        let cfg = ts_rs::Config::default();
        assert_eq!(DhtArc::inline(&cfg), "null | [number, number]");
    }
}
