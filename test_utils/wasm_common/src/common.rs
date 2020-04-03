use holochain_serialized_bytes::prelude::*;

#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct TestString(String);

impl From<String> for TestString {
    fn from(s: String) -> Self {
        Self(s)
    }
}
