use holochain_serialized_bytes::prelude::*;

// Every externed function that the zome developer exposes to holochain returns `ExternIO`.
// The zome developer can expose callbacks in a "sparse" way based on names and the functions
// can take different input (e.g. validation vs. hooks like init, etc.).
// All we can say is that some SerializedBytes are being received and returned.
// In the case of ZomeExtern functions exposed to a client, the data input/output is entirely
// arbitrary so we can't say anything at all. In this case the happ developer must BYO
// deserialization context to match the client, either directly or via. the HDK.
// Note though, that _unlike_ zome externs, the host _does_ know exactly the guest should be
// returning for callbacks, it's just that the unpacking of the return happens in two steps:
// - first the sparse callback is triggered with SB input/output
// - then the guest inflates the expected input or the host the expected output based on the
//   callback flavour

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(transparent)]
#[repr(transparent)]
pub struct ExternIO(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl ExternIO {
    pub fn encode<I>(input: I) -> Result<Self, SerializedBytesError>
    where
        I: serde::Serialize + std::fmt::Debug,
    {
        Ok(Self(holochain_serialized_bytes::encode(&input)?))
    }
    pub fn decode<O>(&self) -> Result<O, SerializedBytesError>
    where
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        holochain_serialized_bytes::decode(&self.0)
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.into()
    }
    pub fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }
}

impl AsRef<[u8]> for ExternIO {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for ExternIO {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl From<ExternIO> for Vec<u8> {
    fn from(extern_io: ExternIO) -> Self {
        extern_io.0
    }
}
