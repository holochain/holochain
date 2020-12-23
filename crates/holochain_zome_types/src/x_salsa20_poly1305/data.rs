/// Data that can be encrypted with secretbox.
#[derive(PartialEq, serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct XSalsa20Poly1305Data(#[serde(with = "serde_bytes")] Vec<u8>);
pub type SecretBoxData = XSalsa20Poly1305Data;
pub type BoxData = XSalsa20Poly1305Data;

impl From<Vec<u8>> for XSalsa20Poly1305Data {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl AsRef<[u8]> for XSalsa20Poly1305Data {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
