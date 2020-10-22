use std::sync::Arc;

/// KitsuneP2p Error Type.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KitsuneP2pError {
    /// GhostError
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),

    /// RoutingSpaceError
    #[error("Routing Space Error: {0:?}")]
    RoutingSpaceError(Arc<KitsuneSpace>),

    /// RoutingAgentError
    #[error("Routing Agent Error: {0:?}")]
    RoutingAgentError(Arc<KitsuneAgent>),

    /// DecodingError
    #[error("Decoding Error: {0}")]
    DecodingError(Arc<String>),

    /// TransportError
    #[error(transparent)]
    TransportError(#[from] kitsune_p2p_types::transport::TransportError),

    /// std::io::Error
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl KitsuneP2pError {
    /// promote a custom error type to a KitsuneP2pError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }

    /// generate a decoding error from a string
    pub fn decoding_error(s: String) -> Self {
        Self::DecodingError(Arc::new(s))
    }
}

impl From<String> for KitsuneP2pError {
    fn from(s: String) -> Self {
        #[derive(Debug, thiserror::Error)]
        struct OtherError(String);
        impl std::fmt::Display for OtherError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        KitsuneP2pError::other(OtherError(s))
    }
}

impl From<&str> for KitsuneP2pError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

/// Kitsune hashes are expected to be 36 bytes.
/// The first 32 bytes are the proper hash.
/// The final 4 bytes are a hash-of-the-hash that can be treated like a u32 "location".
pub trait KitsuneBinType:
    'static
    + Send
    + Sync
    + std::fmt::Debug
    + Clone
    + std::hash::Hash
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + std::convert::From<Vec<u8>>
    + std::convert::Into<Vec<u8>>
{
    /// Fetch just the core 32 bytes (without the 4 location bytes).
    fn get_bytes(&self) -> &[u8];

    /// Fetch the dht "loc" / location for this hash.
    fn get_loc(&self) -> u32;
}

/// internal convert 4 location bytes into a u32 location
fn bytes_to_loc(bytes: &[u8]) -> u32 {
    (bytes[0] as u32)
        + ((bytes[1] as u32) << 8)
        + ((bytes[2] as u32) << 16)
        + ((bytes[3] as u32) << 24)
}

macro_rules! make_kitsune_bin_type {
    ($($doc:expr, $name:ident),*,) => {
        $(
            #[doc = $doc]
            #[derive(
                Clone,
                PartialEq,
                Eq,
                Hash,
                PartialOrd,
                Ord,
                shrinkwraprs::Shrinkwrap,
                derive_more::From,
                derive_more::Into,
                serde::Serialize,
                serde::Deserialize,
            )]
            #[shrinkwrap(mutable)]
            pub struct $name(#[serde(with = "serde_bytes")] pub Vec<u8>);

            impl KitsuneBinType for $name {
                fn get_bytes(&self) -> &[u8] {
                    &self.0[..self.0.len() - 4]
                }

                fn get_loc(&self) -> u32 {
                    bytes_to_loc(&self.0[self.0.len() - 4..])
                }
            }

            impl std::fmt::Debug for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_fmt(format_args!("{}(0x", stringify!($name)))?;
                    for byte in &self.0 {
                        f.write_fmt(format_args!("{:02x}", byte))?;
                    }
                    f.write_fmt(format_args!(")"))?;
                    Ok(())
                }
            }
        )*
    };
}

make_kitsune_bin_type! {
    "Distinguish multiple categories of communication within the same network module.",
    KitsuneSpace,

    "Distinguish multiple agents within the same network module.",
    KitsuneAgent,

    "The basis hash/coordinate when identifying a neighborhood.",
    KitsuneBasis,

    r#"Top-level "KitsuneDataHash" items are buckets of related meta-data.
These metadata "Operations" each also have unique OpHashes."#,
    KitsuneOpHash,
}

/// A cryptographic signature.
#[derive(
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    derive_more::Into,
    serde::Deserialize,
    serde::Serialize,
)]
#[shrinkwrap(mutable)]
pub struct KitsuneSignature(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl std::fmt::Debug for KitsuneSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Signature(0x"))?;
        for byte in &self.0 {
            f.write_fmt(format_args!("{:02x}", byte))?;
        }
        f.write_fmt(format_args!(")"))?;
        Ok(())
    }
}

pub mod actor;
pub mod agent_store;
pub mod event;
pub(crate) mod wire;

pub use kitsune_p2p_types::dht_arc;
