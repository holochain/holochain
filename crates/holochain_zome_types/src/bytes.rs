//! represent arbitrary bytes (not serialized)
//! e.g. totally random crypto bytes from random_bytes

/// simply alias whatever serde bytes is already doing for Vec<u8>
pub type Bytes = serde_bytes::ByteBuf;