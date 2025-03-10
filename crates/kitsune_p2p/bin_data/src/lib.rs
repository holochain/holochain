//! Binary types, hashes, signatures, etc used by kitsune.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use kitsune_p2p_dht_arc::DhtLocation;

#[cfg(feature = "fixt")]
pub mod fixt;

pub mod dependencies {
    #[cfg(feature = "fuzzing")]
    pub use proptest;
    #[cfg(feature = "fuzzing")]
    pub use proptest_derive;
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
    + std::convert::Into<Vec<u8>>
{
    /// Create an instance, ensuring the proper number of bytes were provided.
    fn new(bytes: Vec<u8>) -> Self;

    /// Fetch just the core 32 bytes (without the 4 location bytes).
    fn get_bytes(&self) -> &[u8];

    /// Fetch the dht "loc" / location for this hash.
    fn get_loc(&self) -> DhtLocation;
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
                derive_more::Into,
                serde::Serialize,
                serde::Deserialize,
            )]
            #[shrinkwrap(mutable)]
            pub struct $name(
                #[serde(with = "serde_bytes")]
                pub Vec<u8>);

            impl KitsuneBinType for $name {

                // TODO This actually allows mixups, for example a KitsuneAgent can be constructed with a 36 byte vector
                fn new(mut bytes: Vec<u8>) -> Self {
                    if bytes.len() != 36 {
                        // If location bytes are not included, append them now.
                        debug_assert_eq!(bytes.len(), 32);
                        // FIXME: no way to compute location bytes at this time,
                        // so simply pad with 0's for now
                        bytes.append(&mut [0; 4].to_vec());

                        // todo!("actually calculate location bytes");
                        // bytes.append(&mut kitsune_location_bytes(&bytes));
                    }
                    debug_assert_eq!(bytes.len(), 36);
                    Self(bytes)
                }

                fn get_bytes(&self) -> &[u8] {
                    &self.0[..self.0.len() - 4]
                }

                fn get_loc(&self) -> DhtLocation {
                    DhtLocation::new(bytes_to_loc(&self.0[self.0.len() - 4..]))
                }
            }

            impl std::fmt::Debug for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_fmt(format_args!("{}(0x{})", stringify!($name), &holochain_util::hex::bytes_to_hex(&self.0, false)))
                }
            }

            impl std::fmt::Display for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    URL_SAFE_NO_PAD.encode(&self.0).fmt(f)
                }
            }

            impl AsRef<[u8]> for $name {
                fn as_ref(&self) -> &[u8] {
                    self.0.as_slice()
                }
            }
        )*
    };
}

// #[derive(Debug)]
// pub struct BT(Vec<u8>);

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

/// The op data with its location
#[derive(PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct KitsuneOpData(
    /// The op bytes
    #[serde(with = "serde_bytes")]
    pub Vec<u8>,
);

impl KitsuneOpData {
    /// Constructor
    pub fn new(op: Vec<u8>) -> KOp {
        KOp::new(Self(op))
    }

    /// Size in bytes of this Op
    pub fn size(&self) -> usize {
        self.0.len()
    }
}

impl From<Vec<u8>> for KitsuneOpData {
    fn from(d: Vec<u8>) -> Self {
        Self(d)
    }
}

impl std::fmt::Debug for KitsuneOpData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "KitsuneOpData({})",
            &holochain_util::hex::many_bytes_string(self.0.as_slice())
        ))
    }
}

/// Convenience type
pub type KOp = std::sync::Arc<KitsuneOpData>;

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

/// A 32 byte cert identifying a peer
#[derive(
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::Deref,
    derive_more::From,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct NodeCert(std::sync::Arc<[u8; 32]>);

impl std::fmt::Debug for NodeCert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("NodeCert")
            .field(&holochain_util::hex::many_bytes_string(self.0.as_slice()))
            .finish()
    }
}
