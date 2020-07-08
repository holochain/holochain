use hdk3::prelude::*;

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct TestString(pub String);

impl From<String> for TestString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct TestBytes(#[serde(with = "serde_bytes")] Vec<u8>);

impl From<Vec<u8>> for TestBytes {
    fn from(b: Vec<u8>) -> Self {
        Self(b)
    }
}

#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct TestBool(pub bool);

impl From<bool> for TestBool {
    fn from(b: bool) -> Self {
        Self(b)
    }
}

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AnchorInput(pub String, pub String);

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
pub struct MaybeAnchor(pub Option<Anchor>);
