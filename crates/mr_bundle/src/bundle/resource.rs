/// Opaque bytes representing a Resource in a [`Bundle`](crate::Bundle)
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "api/admin/types.ts"))]
pub struct ResourceBytes(#[cfg_attr(feature = "ts_rs", ts(type = "Uint8Array"))] bytes::Bytes);

impl std::fmt::Debug for ResourceBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "mr_bundle::ResourceBytes({})",
            &holochain_util::hex::many_bytes_string(&self.0)
        ))
    }
}

impl From<bytes::Bytes> for ResourceBytes {
    fn from(value: bytes::Bytes) -> Self {
        Self(value)
    }
}

impl From<ResourceBytes> for bytes::Bytes {
    fn from(value: ResourceBytes) -> Self {
        value.0
    }
}

impl From<Vec<u8>> for ResourceBytes {
    fn from(value: Vec<u8>) -> Self {
        Self(value.into())
    }
}

impl AsRef<[u8]> for ResourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}
