//! Simple wrappers around the basic rkv Database types, providing strong typing
//! through automatic de/serialization

mod kv;

pub use kv::*;


/// Use this as the key type for LMDB databases which should only have one key.
///
/// This type can only be used as one possible reference, the empty byte slice
#[derive(Hash, PartialEq, Eq)]
pub struct UnitDbKey;

impl AsRef<[u8]> for UnitDbKey {
    fn as_ref(&self) -> &[u8] {
        ARBITRARY_BYTE_SLICE
    }
}

static ARBITRARY_BYTE_SLICE: &[u8] = &[0];
