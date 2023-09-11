/// Arbitrary opaque bytes representing a Resource in a [`Bundle`](crate::Bundle)
#[derive(
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    derive_more::From,
    derive_more::Deref,
)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct ResourceBytes(#[serde(with = "serde_bytes")] Vec<u8>);

impl ResourceBytes {
    /// Accessor
    pub fn inner(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Convert to raw vec
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl std::fmt::Debug for ResourceBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "mr_bundle::ResourceBytes({})",
            &holochain_util::hex::many_bytes_string(self.0.as_slice())
        ))
    }
}
