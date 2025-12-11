use ts_rs::TS;
use export_types_config::EXPORT_TS_TYPES_FILE;

/// Data that can be encrypted with secretbox.
#[derive(PartialEq, Eq, serde::Serialize, serde::Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = EXPORT_TS_TYPES_FILE)]
pub struct XSalsa20Poly1305Data(#[serde(with = "serde_bytes")] Vec<u8>);

#[derive(TS)]
pub type SecretBoxData = XSalsa20Poly1305Data;

#[derive(TS)]
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
