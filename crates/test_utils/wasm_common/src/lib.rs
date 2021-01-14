use hdk3::prelude::*;

#[derive(Serialize, Deserialize, SerializedBytes)]
#[repr(transparent)]
pub struct TestBytes(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl From<Vec<u8>> for TestBytes {
    fn from(b: Vec<u8>) -> Self {
        Self(b)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AnchorInput(pub String, pub String);

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AgentActivitySearch {
    pub agent: AgentPubKey,
    pub query: QueryFilter,
    pub request: ActivityRequest,
}
