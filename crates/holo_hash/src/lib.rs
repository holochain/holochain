//! Defines HoloHash and its various HashTypes

#![deny(missing_docs)]

mod aliases;
pub mod error;
mod has_hash;
mod hash;
pub mod hash_type;

pub use aliases::*;
pub use has_hash::HasHash;
pub use hash::*;
pub use hash_type::HashType;
pub use hash_type::PrimitiveHashType;

// feature: serialization (enabled by default)
// (serde, SerializedBytes)

#[cfg(feature = "serialization")]
mod hashed;
#[cfg(feature = "serialization")]
pub use hashed::*;

#[cfg(feature = "serialization")]
mod hashable_content;
#[cfg(feature = "serialization")]
pub use hashable_content::*;

#[cfg(feature = "serialization")]
mod ser;

#[cfg(feature = "serialization")]
/// A convenience type, for specifying a hash by HashableContent rather than
/// by its HashType
pub type HoloHashOf<C> = HoloHash<<C as HashableContent>::HashType>;

// feature: encoding
// (string encoding)

#[cfg(feature = "encoding")]
pub use encode::{holo_hash_decode, holo_hash_decode_unchecked, holo_hash_encode};

/// By default, disable string encoding and just display raw bytes
#[cfg(not(feature = "encoding"))]
pub mod encode_raw;

/// Include nice string encoding methods and From impls
#[cfg(feature = "encoding")]
pub mod encode;

#[cfg(feature = "encoding")]
mod hash_b64;
#[cfg(feature = "encoding")]
pub use hash_b64::*;

// feature: hashing
// (blake2b hashing for hash generation and DHT location calculation)

#[cfg(feature = "hashing")]
mod hash_ext;

#[cfg(feature = "hashing")]
pub use hash_ext::*;

// feature: fixturators
// provides fixturators for all hash types
#[cfg(feature = "fixturators")]
pub mod fixt;
