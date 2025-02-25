use crate::*;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
/// Struct for encoding DhtOp as bytes.
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
    RemoteSignalEvt {
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    },
    GetReq {
        msg_id: u64,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetOptions,
    },
    GetRes {
        msg_id: u64,
        response: WireOps,
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
    /*
    ValidationReceipts {
        receipts: ValidationReceiptBundle,
    },
    GetAgentActivity {
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: event::GetActivityOptions,
    },
    MustGetAgentActivity {
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    },
    CountersigningSessionNegotiation {
        message: event::CountersigningSessionNegotiationMessage,
    },
    PublishCountersign {
        flag: bool,
        op: DhtOp,
    },
    */
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

    /// Outgoing "Get" request.
    pub fn get_req(
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetOptions,
    ) -> (u64, WireMessage) {
        let msg_id = next_msg_id();
        (
            msg_id,
            Self::GetReq {
                msg_id,
                to_agent,
                dht_hash,
                options,
            },
        )
    }

    /// Incoming "Get" response.
    pub fn get_res(msg_id: u64, response: WireOps) -> WireMessage {
        Self::GetRes { msg_id, response }
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

    /*
    pub fn validation_receipts(receipts: ValidationReceiptBundle) -> WireMessage {
        Self::ValidationReceipts { receipts }
    }




    pub fn get_agent_activity(
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: event::GetActivityOptions,
    ) -> WireMessage {
        Self::GetAgentActivity {
            agent,
            query,
            options,
        }
    }

    pub fn must_get_agent_activity(
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> WireMessage {
        Self::MustGetAgentActivity { agent, filter }
    }

    pub fn countersigning_session_negotiation(
        message: event::CountersigningSessionNegotiationMessage,
    ) -> WireMessage {
        Self::CountersigningSessionNegotiation { message }
    }

    pub fn publish_countersign(flag: bool, op: DhtOp) -> WireMessage {
        Self::PublishCountersign { flag, op }
    }
    */
}
