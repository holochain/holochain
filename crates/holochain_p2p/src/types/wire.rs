use crate::*;

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(tag = "type", content = "content")]
pub(crate) enum WireMessage {
    CallRemote {
        zome_name: String,
        fn_name: String,
        cap: String,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
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

    pub fn call_remote(zome_name: String, fn_name: String, cap: String, request: SerializedBytes) -> WireMessage {
        Self::CallRemote {
            zome_name,
            fn_name,
            cap,
            data: UnsafeBytes::from(request).into(),
        }
    }

    pub fn validation_receipt(receipt: SerializedBytes) -> WireMessage {
        Self::ValidationReceipt {
            receipt: UnsafeBytes::from(receipt).into(),
        }
    }
}
