//! Simple wrappers around the basic rkv Database types, providing strong typing
//! through automatic de/serialization

mod kv;
use derive_more::Display;

pub use kv::*;

/// Use this as the key type for LMDB databases which should only have one key.
///
/// This type can only be used as one possible reference
#[derive(Display, Hash, PartialEq, Eq)]
pub struct UnitDbKey;

impl AsRef<[u8]> for UnitDbKey {
    fn as_ref(&self) -> &[u8] {
        ARBITRARY_BYTE_SLICE
    }
}

impl From<()> for UnitDbKey {
    fn from(_: ()) -> Self {
        Self
    }
}

static ARBITRARY_BYTE_SLICE: &[u8] = &[0];

/// A zero-size value type. Useful when inspecting databases without caring
/// about their actual values.
#[derive(
    Display, Hash, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize, std::fmt::Debug,
)]
pub struct UnitDbVal;
