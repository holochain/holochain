//! A UTC timestamp for use in Holochain's headers.
//!
//! Includes a struct that gives a uniform well-ordered byte representation
//! of a timestamp, used for chronologically ordered database keys

/// A UTC timestamp for use in Holochain's headers.
///
/// Timestamp implements `Serialize` and `Display` as rfc3339 time strings.
/// - Field 0: i64 - Seconds since UNIX epoch UTC (midnight 1970-01-01).
/// - Field 1: u32 - Nanoseconds in addition to above seconds.
///
/// Supports +/- std::time::Duration directly
pub use holochain_zome_types::timestamp::*; // Timestamp, TimestampError

/// Returns the current system time as a Timestamp.  We do not make this a holochain_zome_types
/// timestamp::Timestamp impl now() method, because we need Timestamp to be WASM compatible, and
/// chrono doesn't have a now() implementation for WASM.  So, use holochain_types timestamp::now()
/// instead.
pub fn now() -> Timestamp {
    Timestamp::from(chrono::offset::Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_serialized_bytes::prelude::*;
    use std::convert::TryInto;

    const TEST_TS: &'static str = "2020-05-05T19:16:04.266431045Z";
    const TEST_EN: &'static [u8] = b"\x92\xce\x5e\xb1\xbb\x74\xce\x0f\xe1\x6a\x45";

    #[test]
    fn test_timestamp_serialization() {
        let t: Timestamp = TEST_TS.try_into().unwrap();
        assert_eq!(t.secs(), 1588706164);
        assert_eq!(t.nsecs(), 266431045);
        assert_eq!(TEST_TS, &t.to_string());

        #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
        struct S(Timestamp);
        let s = S(t);
        let sb = SerializedBytes::try_from(s).unwrap();
        assert_eq!(&TEST_EN[..], sb.bytes().as_slice());

        let s: S = sb.try_into().unwrap();
        let t = s.0;
        assert_eq!(TEST_TS, &t.to_string());
    }
}
