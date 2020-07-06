use holochain_serialized_bytes::prelude::*;

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
