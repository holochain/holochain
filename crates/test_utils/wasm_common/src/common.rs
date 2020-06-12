use holochain_serialized_bytes::prelude::*;

#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct TestString(String);

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
