//! # Timestamp

use holochain_serialized_bytes::prelude::*;

/// A UTC timestamp for use in Holochain's headers.
///
/// Timestamp implements `Serialize` and `Display` as rfc3339 time strings.
/// - Field 0: i64 - Seconds since UNIX epoch UTC (midnight 1970-01-01).
/// - Field 1: u32 - Nanoseconds in addition to above seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(
    // sec
    pub i64,
    // nsec
    pub u32,
);
