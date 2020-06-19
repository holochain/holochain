pub struct Bytes(#[serde(with = "serde_bytes")] Vec<u8>);

impl From<Vec<u8>> for Bytes {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}
