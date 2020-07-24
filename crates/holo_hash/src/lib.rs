//! Defines HoloHashOf and its various HashTypes

#![deny(missing_docs)]

mod aliases;
pub mod error;
mod has_hash;
mod hash;
pub mod hash_type;
mod hashable_content;
mod hashed;
mod ser;

pub use aliases::*;
pub use has_hash::HasHash;
pub use hash::*;
pub use hash_type::{HashType, PrimitiveHashType};
pub use hashable_content::*;
pub use hashed::*;

#[cfg(feature = "hashing")]
mod hash_ext;
#[cfg(feature = "hashing")]
mod hashed_ext;
#[cfg(feature = "fixturators")]
pub mod fixt;

/// By default, disable string encoding and just display raw bytes
#[cfg(not(feature = "string-encoding"))]
pub mod encode_raw;

/// Include nice string encoding methods and From impls
#[cfg(feature = "string-encoding")]
pub mod encode;

/// A convenience type, for specifying a hash by HashableContent rather than
/// by its HashType
pub type HoloHashOf<C> = HoloHash<<C as HashableContent>::HashType>;
