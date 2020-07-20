//! Defines HoloHash and its various HashTypes

#![deny(missing_docs)]

mod aliases;
pub mod error;
mod has_hash;
mod hash;
pub mod hash_type;
mod hashable_content;
mod ser;
// mod serialize_hash_type;
pub use aliases::*;
pub use has_hash::HasHash;
pub use hash::*;
pub use hash_type::{HashType, PrimitiveHashType};
pub use hashable_content::*;

/// By default, disable string encoding and just display raw bytes
#[cfg(not(feature = "string-encoding"))]
pub mod encode_raw;

/// Include nice string encoding methods and From impls
#[cfg(feature = "string-encoding")]
pub mod encode;
