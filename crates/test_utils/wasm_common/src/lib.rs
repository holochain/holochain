use hdk3::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
pub struct TestString(pub String);

impl From<String> for TestString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TestString {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

#[derive(Serialize, Deserialize, SerializedBytes)]
#[repr(transparent)]
pub struct TestBytes(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl From<Vec<u8>> for TestBytes {
    fn from(b: Vec<u8>) -> Self {
        Self(b)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
pub struct TestBool(pub bool);

impl From<bool> for TestBool {
    fn from(b: bool) -> Self {
        Self(b)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct TestInt(pub u32);

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AnchorInput(pub String, pub String);

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
pub struct MaybeAnchor(pub Option<Anchor>);

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
pub struct LinkTags(pub Vec<LinkTag>);

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
pub struct AnchorTags(pub Vec<String>);

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AgentActivitySearch {
    pub agent: AgentPubKey,
    pub query: QueryFilter,
    pub request: ActivityRequest,
}
