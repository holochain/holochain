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

#[cfg(any(feature = "string-encoding"))]
pub use encode::{holo_hash_decode, holo_hash_decode_unchecked, holo_hash_encode};

/// By default, disable string encoding and just display raw bytes
#[cfg(not(feature = "string-encoding"))]
pub mod encode_raw;

/// Include nice string encoding methods and From impls
#[cfg(feature = "string-encoding")]
pub mod encode;

#[cfg(feature = "string-encoding")]
pub mod hash_b64;

#[cfg(feature = "string-encoding")]
pub use hash_b64::*;

#[cfg(feature = "fixturators")]
pub mod fixt;

#[cfg(feature = "hashing")]
mod hash_ext;

#[cfg(feature = "serialized-bytes")]
mod hashable_content;
#[cfg(feature = "serialized-bytes")]
mod hashed;
#[cfg(feature = "serialized-bytes")]
mod ser;

#[cfg(feature = "hashing")]
pub use hash_ext::*;
#[cfg(feature = "serialized-bytes")]
pub use hashable_content::*;
#[cfg(feature = "serialized-bytes")]
pub use hashed::*;

/// A convenience type, for specifying a hash by HashableContent rather than
/// by its HashType
#[cfg(feature = "serialized-bytes")]
pub type HoloHashOf<C> = HoloHash<<C as HashableContent>::HashType>;
