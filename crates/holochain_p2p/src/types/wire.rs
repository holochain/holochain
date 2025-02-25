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
    /*
    ValidationReceipts {
        receipts: ValidationReceiptBundle,
    },
    GetMeta {
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetMetaOptions,
    },
    GetLinks {
        link_key: WireLinkKey,
        options: event::GetLinksOptions,
    },
    CountLinks {
        query: WireLinkQuery,
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

    /*
    pub fn validation_receipts(receipts: ValidationReceiptBundle) -> WireMessage {
        Self::ValidationReceipts { receipts }
    }


    pub fn get_meta(
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetMetaOptions,
    ) -> WireMessage {
        Self::GetMeta { dht_hash, options }
    }

    pub fn get_links(link_key: WireLinkKey, options: event::GetLinksOptions) -> WireMessage {
        Self::GetLinks { link_key, options }
    }

    pub fn count_links(query: WireLinkQuery) -> WireMessage {
        Self::CountLinks { query }
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
