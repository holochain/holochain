use crate::*;

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "content")]
pub(crate) enum WireMessage {
    CallRemote {
        zome_name: ZomeName,
        fn_name: String,
        cap: CapSecret,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    Publish {
        from_agent: holo_hash::AgentPubKey,
        request_validation_receipt: bool,
        dht_hash: holochain_types::composite_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    },
    ValidationReceipt {
        #[serde(with = "serde_bytes")]
        receipt: Vec<u8>,
    },
    Get {
        dht_hash: holochain_types::composite_hash::AnyDhtHash,
        options: event::GetOptions,
    },
    GetLinks {
        dht_hash: holochain_types::composite_hash::AnyDhtHash,
        options: event::GetLinksOptions,
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

    pub fn call_remote(
        zome_name: ZomeName,
        fn_name: String,
        cap: CapSecret,
        request: SerializedBytes,
    ) -> WireMessage {
        Self::CallRemote {
            zome_name,
            fn_name,
            cap,
            data: UnsafeBytes::from(request).into(),
        }
    }

    pub fn publish(
        from_agent: holo_hash::AgentPubKey,
        request_validation_receipt: bool,
        dht_hash: holochain_types::composite_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> WireMessage {
        Self::Publish {
            from_agent,
            request_validation_receipt,
            dht_hash,
            ops,
        }
    }

    pub fn validation_receipt(receipt: SerializedBytes) -> WireMessage {
        Self::ValidationReceipt {
            receipt: UnsafeBytes::from(receipt).into(),
        }
    }

    pub fn get(
        dht_hash: holochain_types::composite_hash::AnyDhtHash,
        options: event::GetOptions,
    ) -> WireMessage {
        Self::Get { dht_hash, options }
    }

    pub fn get_links(
        dht_hash: holochain_types::composite_hash::AnyDhtHash,
        options: event::GetLinksOptions,
    ) -> WireMessage {
        Self::GetLinks { dht_hash, options }
    }
}
