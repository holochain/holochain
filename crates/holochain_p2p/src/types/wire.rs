use crate::*;
use holochain_zome_types::zome::FunctionName;

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
        zome_name: ZomeName,
        fn_name: FunctionName,
        from_agent: holo_hash::AgentPubKey,
        cap_secret: Option<CapSecret>,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    Publish {
        request_validation_receipt: bool,
        countersigning_session: bool,
        dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<holochain_types::dht_op::DhtOp>,
    },
    ValidationReceipt {
        #[serde(with = "serde_bytes")]
        receipt: Vec<u8>,
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
    GetAgentActivity {
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: event::GetActivityOptions,
    },
    GetValidationPackage {
        action_hash: ActionHash,
    },
    CountersigningSessionNegotiation {
        message: event::CountersigningSessionNegotiationMessage,
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

    pub fn call_remote(
        zome_name: ZomeName,
        fn_name: FunctionName,
        from_agent: holo_hash::AgentPubKey,
        cap_secret: Option<CapSecret>,
        payload: ExternIO,
    ) -> WireMessage {
        Self::CallRemote {
            zome_name,
            fn_name,
            from_agent,
            cap_secret,
            data: payload.into_vec(),
        }
    }

    pub fn publish(
        request_validation_receipt: bool,
        countersigning_session: bool,
        dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> WireMessage {
        Self::Publish {
            request_validation_receipt,
            countersigning_session,
            dht_hash,
            ops,
        }
    }

    pub fn validation_receipt(receipt: SerializedBytes) -> WireMessage {
        Self::ValidationReceipt {
            receipt: UnsafeBytes::from(receipt).into(),
        }
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
    pub fn get_validation_package(action_hash: ActionHash) -> WireMessage {
        Self::GetValidationPackage { action_hash }
    }

    pub fn countersigning_session_negotiation(
        message: event::CountersigningSessionNegotiationMessage,
    ) -> WireMessage {
        Self::CountersigningSessionNegotiation { message }
    }
}
