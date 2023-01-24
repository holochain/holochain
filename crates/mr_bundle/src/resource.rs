/// Arbitrary opaque bytes representing a Resource in a [`Bundle`](crate::Bundle)
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    derive_more::From,
    derive_more::Deref,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
