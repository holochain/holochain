use crate::*;

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// Struct for encoding DhtOp as bytes.
pub struct WireDhtOpData {
    /// The dht op.
    pub op_data: holochain_types::dht_op::DhtOp,
}

impl WireDhtOpData {
    /// Encode as bytes.
    pub fn encode(self) -> Result<Vec<u8>, SerializedBytesError> {
        Ok(UnsafeBytes::from(SerializedBytes::try_from(self)?).into())
    }

    /// Decode from bytes.
    pub fn decode(data: Vec<u8>) -> Result<Self, SerializedBytesError> {
        let request: SerializedBytes = UnsafeBytes::from(data).into();
        request.try_into()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "content")]
#[allow(missing_docs)]
pub enum WireMessage {
    CallRemote {
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    },
    CallRemoteMulti {
        to_agents: Vec<(holo_hash::AgentPubKey, ExternIO, Signature)>,
    },
    ValidationReceipts {
        receipts: ValidationReceiptBundle,
    },
    Get {
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetOptions,
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
}

#[allow(missing_docs)]
impl WireMessage {
    pub fn encode(&self) -> Result<Vec<u8>, SerializedBytesError> {
        holochain_serialized_bytes::encode(&self)
    }

    pub fn decode(data: &[u8]) -> Result<Self, SerializedBytesError> {
        holochain_serialized_bytes::decode(&data)
    }

    pub fn publish_countersign(flag: bool, op: DhtOp) -> WireMessage {
        Self::PublishCountersign { flag, op }
    }

    /// For an outgoing remote call.
    pub fn call_remote(
        to_agent: holo_hash::AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> WireMessage {
        Self::CallRemote {
            to_agent,
            zome_call_params_serialized,
            signature,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn call_remote_multi(
        to_agents: Vec<(holo_hash::AgentPubKey, ExternIO, Signature)>,
    ) -> WireMessage {
        Self::CallRemoteMulti { to_agents }
    }

    pub fn validation_receipts(receipts: ValidationReceiptBundle) -> WireMessage {
        Self::ValidationReceipts { receipts }
    }

    pub fn get(dht_hash: holo_hash::AnyDhtHash, options: event::GetOptions) -> WireMessage {
        Self::Get { dht_hash, options }
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
}
