//! Signature for authenticity of data 
use holochain_serialized_bytes::prelude::*;

/// The raw bytes of a signature.
#[derive(
    Clone,
    Serialize,
    Deserialize,
    SerializedBytes,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct Signature(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Signature(0x"))?;
        for byte in &self.0 {
            f.write_fmt(format_args!("{:02x}", byte))?;
        }
        f.write_fmt(format_args!(")"))?;
        Ok(())
    }
}
