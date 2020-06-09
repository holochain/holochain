use crate::*;

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "content")]
pub(crate) enum WireMessage {
    CallRemote {
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    Publish {
        from_agent: holo_hash::AgentPubKey,
        request_validation_receipt: bool,
        entry_hash: holochain_types::composite_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dhtops::DhtOp)>,
    },
    ValidationReceipt {
        #[serde(with = "serde_bytes")]
        receipt: Vec<u8>,
    },
}

impl WireMessage {
    pub fn encode(self) -> Result<Vec<u8>, SerializedBytesError> {
        Ok(UnsafeBytes::from(SerializedBytes::try_from(self)?).into())
    }

    pub fn decode(data: Vec<u8>) -> Result<Self, SerializedBytesError> {
        let request: SerializedBytes = UnsafeBytes::from(data).into();
        Ok(request.try_into()?)
    }

    pub fn call_remote(request: SerializedBytes) -> WireMessage {
        Self::CallRemote {
            data: UnsafeBytes::from(request).into(),
        }
    }

    pub fn publish(
        from_agent: holo_hash::AgentPubKey,
        request_validation_receipt: bool,
        entry_hash: holochain_types::composite_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dhtops::DhtOp)>,
    ) -> WireMessage {
        Self::Publish {
            from_agent,
            request_validation_receipt,
            entry_hash,
            ops,
        }
    }

    pub fn validation_receipt(receipt: SerializedBytes) -> WireMessage {
        Self::ValidationReceipt {
            receipt: UnsafeBytes::from(receipt).into(),
        }
    }
}
