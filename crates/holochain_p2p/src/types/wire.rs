use crate::*;

/// Struct for encoding DhtOp as bytes.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WireDhtOpData {
    /// The dht op.
    pub op_data: holochain_types::dht_op::DhtOp,
}

impl WireDhtOpData {
    /// Encode as bytes.
    pub fn encode(self) -> Result<bytes::Bytes, HolochainP2pError> {
        let mut b = bytes::BufMut::writer(bytes::BytesMut::new());
        rmp_serde::encode::write_named(&mut b, &self).map_err(HolochainP2pError::other)?;
        Ok(b.into_inner().freeze())
    }

    /// Decode from bytes.
    pub fn decode(data: &[u8]) -> Result<Self, HolochainP2pError> {
        rmp_serde::decode::from_slice(data).map_err(HolochainP2pError::other)
    }
}

/// Encoding for the hcp2p preflight message.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WirePreflightMessage {
    /// Compatibility parameters.
    pub compat: NetworkCompatParams,
    /// Local agent infos.
    pub agents: Vec<String>,
}

impl WirePreflightMessage {
    /// Encode.
    pub fn encode(&self) -> Result<bytes::Bytes, HolochainP2pError> {
        let mut b = bytes::BufMut::writer(bytes::BytesMut::new());
        rmp_serde::encode::write_named(&mut b, self).map_err(HolochainP2pError::other)?;
        Ok(b.into_inner().freeze())
    }

    /// Decode.
    pub fn decode(data: &[u8]) -> Result<Self, HolochainP2pError> {
        rmp_serde::decode::from_slice(data).map_err(HolochainP2pError::other)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "content")]
#[allow(missing_docs)]
pub enum WireMessage {
    ErrorRes {
        msg_id: u64,
        error: String,
    },
    CallRemoteReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    },
    CallRemoteRes {
        msg_id: u64,
        response: SerializedBytes,
    },
    GetReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
    },
    GetRes {
        msg_id: u64,
        response: WireOps,
    },
    GetByOpTypeReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        action_hash: ActionHash,
        op_type: ChainOpType,
    },
    GetByOpTypeRes {
        msg_id: u64,
        response: WireMaybeOpByType,
    },
    GetMetaReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetMetaOptions,
    },
    GetMetaRes {
        msg_id: u64,
        response: MetadataSet,
    },
    GetLinksReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: event::GetLinksOptions,
    },
    GetLinksRes {
        msg_id: u64,
        response: WireLinkOps,
    },
    CountLinksReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        query: WireLinkQuery,
    },
    CountLinksRes {
        msg_id: u64,
        response: CountLinksResponse,
    },
    GetAgentActivityReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: event::GetActivityOptions,
    },
    GetAgentActivityRes {
        msg_id: u64,
        response: AgentActivityResponse,
    },
    MustGetAgentActivityReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    },
    MustGetAgentActivityRes {
        msg_id: u64,
        response: MustGetAgentActivityResponse,
    },
    SendValidationReceiptsReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    },
    SendValidationReceiptsRes {
        msg_id: u64,
    },
    RemoteSignalEvt {
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    },
    PublishCountersignEvt {
        op: ChainOp,
    },
    CountersigningSessionNegotiationEvt {
        to_agent: AgentPubKey,
        message: event::CountersigningSessionNegotiationMessage,
    },
}

fn next_msg_id() -> u64 {
    static M: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    M.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[allow(missing_docs)]
impl WireMessage {
    pub fn encode_batch(batch: &[&WireMessage]) -> Result<bytes::Bytes, HolochainP2pError> {
        let mut b = bytes::BufMut::writer(bytes::BytesMut::new());
        rmp_serde::encode::write_named(&mut b, batch).map_err(HolochainP2pError::other)?;
        Ok(b.into_inner().freeze())
    }

    pub fn decode_batch(data: &[u8]) -> Result<Vec<Self>, HolochainP2pError> {
        rmp_serde::decode::from_slice(data).map_err(HolochainP2pError::other)
    }

    pub fn get_msg_id(&self) -> Option<u64> {
        match self {
            WireMessage::ErrorRes { msg_id, .. } => Some(*msg_id),
            WireMessage::CallRemoteReq { msg_id, .. } => Some(*msg_id),
            WireMessage::CallRemoteRes { msg_id, .. } => Some(*msg_id),
            WireMessage::GetReq { msg_id, .. } => Some(*msg_id),
            WireMessage::GetRes { msg_id, .. } => Some(*msg_id),
            WireMessage::GetByOpTypeReq { msg_id, .. } => Some(*msg_id),
            WireMessage::GetByOpTypeRes { msg_id, .. } => Some(*msg_id),
            WireMessage::GetMetaReq { msg_id, .. } => Some(*msg_id),
            WireMessage::GetMetaRes { msg_id, .. } => Some(*msg_id),
            WireMessage::GetLinksReq { msg_id, .. } => Some(*msg_id),
            WireMessage::GetLinksRes { msg_id, .. } => Some(*msg_id),
            WireMessage::CountLinksReq { msg_id, .. } => Some(*msg_id),
            WireMessage::CountLinksRes { msg_id, .. } => Some(*msg_id),
            WireMessage::GetAgentActivityReq { msg_id, .. } => Some(*msg_id),
            WireMessage::GetAgentActivityRes { msg_id, .. } => Some(*msg_id),
            WireMessage::MustGetAgentActivityReq { msg_id, .. } => Some(*msg_id),
            WireMessage::MustGetAgentActivityRes { msg_id, .. } => Some(*msg_id),
            WireMessage::SendValidationReceiptsReq { msg_id, .. } => Some(*msg_id),
            WireMessage::SendValidationReceiptsRes { msg_id, .. } => Some(*msg_id),
            _ => None,
        }
    }

    /// Outgoing "CallRemote" request.
    pub fn call_remote_req(
        to_agent: holo_hash::AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::CallRemoteReq {
                msg_id,
                to_agent,
                zome_call_params_serialized,
                signature,
            },
        )
    }

    /// Incoming "CallRemote" response.
    pub fn call_remote_res(msg_id: u64, response: SerializedBytes) -> WireMessage {
        Self::CallRemoteRes { msg_id, response }
    }

    /// Outgoing "Get" request.
    pub fn get_req(to_agent: AgentPubKey, dht_hash: holo_hash::AnyDhtHash) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::GetReq {
                msg_id,
                to_agent,
                dht_hash,
            },
        )
    }

    /// Incoming "Get" response.
    pub fn get_res(msg_id: u64, response: WireOps) -> WireMessage {
        Self::GetRes { msg_id, response }
    }

    /// Outgoing "GetByOpType" request.
    pub fn get_by_op_type_req(
        to_agent: AgentPubKey,
        action_hash: ActionHash,
        op_type: ChainOpType,
    ) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::GetByOpTypeReq {
                msg_id,
                to_agent,
                action_hash,
                op_type,
            },
        )
    }

    /// Incoming "GetByOpType" response.
    pub fn get_by_op_type_res(msg_id: u64, response: WireMaybeOpByType) -> WireMessage {
        Self::GetByOpTypeRes { msg_id, response }
    }

    /// Outgoing "GetMeta" request.
    pub fn get_meta_req(
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetMetaOptions,
    ) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::GetMetaReq {
                msg_id,
                to_agent,
                dht_hash,
                options,
            },
        )
    }

    /// Incoming "GetMeta" response.
    pub fn get_meta_res(msg_id: u64, response: MetadataSet) -> WireMessage {
        Self::GetMetaRes { msg_id, response }
    }

    /// Outgoing "GetLinks" request.
    pub fn get_links_req(
        to_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: event::GetLinksOptions,
    ) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::GetLinksReq {
                msg_id,
                to_agent,
                link_key,
                options,
            },
        )
    }

    /// Incoming "GetLinks" response.
    pub fn get_links_res(msg_id: u64, response: WireLinkOps) -> WireMessage {
        Self::GetLinksRes { msg_id, response }
    }

    /// Outgoing "CountLinks" request.
    pub fn count_links_req(to_agent: AgentPubKey, query: WireLinkQuery) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::CountLinksReq {
                msg_id,
                to_agent,
                query,
            },
        )
    }

    /// Incoming "CountLinks" response.
    pub fn count_links_res(msg_id: u64, response: CountLinksResponse) -> WireMessage {
        Self::CountLinksRes { msg_id, response }
    }

    /// Outgoing "GetAgentActivity" request.
    pub fn get_agent_activity_req(
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: event::GetActivityOptions,
    ) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::GetAgentActivityReq {
                msg_id,
                to_agent,
                agent,
                query,
                options,
            },
        )
    }

    /// Incoming "GetAgentActivity" response.
    pub fn get_agent_activity_res(msg_id: u64, response: AgentActivityResponse) -> WireMessage {
        Self::GetAgentActivityRes { msg_id, response }
    }

    /// Outgoing "MustGetAgentActivity" request.
    pub fn must_get_agent_activity_req(
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::MustGetAgentActivityReq {
                msg_id,
                to_agent,
                agent,
                filter,
            },
        )
    }

    /// Incoming "MustGetAgentActivity" response.
    pub fn must_get_agent_activity_res(
        msg_id: u64,
        response: MustGetAgentActivityResponse,
    ) -> WireMessage {
        Self::MustGetAgentActivityRes { msg_id, response }
    }

    /// Outgoing "SendValidationReceipts" request.
    pub fn send_validation_receipts_req(
        to_agent: holo_hash::AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::SendValidationReceiptsReq {
                msg_id,
                to_agent,
                receipts,
            },
        )
    }

    /// Incoming "SendValidationReceipts" response.
    pub fn send_validation_receipts_res() -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (msg_id, Self::SendValidationReceiptsRes { msg_id })
    }

    /// Outgoing fire-and-forget "RemoteSignal" notify event.
    pub fn remote_signal_evt(
        to_agent: holo_hash::AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> WireMessage {
        Self::RemoteSignalEvt {
            to_agent,
            zome_call_params_serialized,
            signature,
        }
    }

    /// Outgoing "PublishCountersign" notify event.
    pub fn publish_countersign_evt(op: ChainOp) -> WireMessage {
        Self::PublishCountersignEvt { op }
    }

    /// Outgoing "CountersigningSessionNegotiation" notify event.
    pub fn countersigning_session_negotiation_evt(
        to_agent: AgentPubKey,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> WireMessage {
        Self::CountersigningSessionNegotiationEvt { to_agent, message }
    }
}
