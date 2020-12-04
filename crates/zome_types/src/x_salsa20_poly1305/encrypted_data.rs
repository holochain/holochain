#[derive(PartialEq, serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct XSalsa20Poly1305EncryptedData(#[serde(with = "serde_bytes")] Vec<u8>);
pub type SecretBoxEncryptedData = XSalsa20Poly1305EncryptedData;

impl From<Vec<u8>> for XSalsa20Poly1305EncryptedData {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl AsRef<[u8]> for XSalsa20Poly1305EncryptedData {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
