//! A UTC timestamp for use in Holochain's headers.
//!
//! Includes a struct that gives a uniform well-ordered byte representation
//! of a timestamp, used for chronologically ordered database keys

use std::convert::TryFrom;
use std::convert::TryInto;

/// A UTC timestamp for use in Holochain's headers.
///
/// Timestamp implements `Serialize` and `Display` as rfc3339 time strings.
/// - Field 0: i64 - Seconds since UNIX epoch UTC (midnight 1970-01-01).
/// - Field 1: u32 - Nanoseconds in addition to above seconds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct Timestamp(
    // sec
    pub i64,
    // nsec
    pub u32,
);

impl Timestamp {
    /// Create a new Timestamp instance from current system time.
    pub fn now() -> Self {
        chrono::offset::Utc::now().into()
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t: chrono::DateTime<chrono::Utc> = self.into();
        write!(
            f,
            "{}",
            t.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
        )
    }
}

impl From<chrono::DateTime<chrono::Utc>> for Timestamp {
    fn from(t: chrono::DateTime<chrono::Utc>) -> Self {
        std::convert::From::from(&t)
    }
}

impl From<&chrono::DateTime<chrono::Utc>> for Timestamp {
    fn from(t: &chrono::DateTime<chrono::Utc>) -> Self {
        let t = t.naive_utc();
        Timestamp(t.timestamp(), t.timestamp_subsec_nanos())
    }
}

impl From<Timestamp> for chrono::DateTime<chrono::Utc> {
    fn from(t: Timestamp) -> Self {
        std::convert::From::from(&t)
    }
}

impl From<&Timestamp> for chrono::DateTime<chrono::Utc> {
    fn from(t: &Timestamp) -> Self {
        let t = chrono::naive::NaiveDateTime::from_timestamp(t.0, t.1);
        chrono::DateTime::from_utc(t, chrono::Utc)
    }
}

impl std::convert::TryFrom<String> for Timestamp {
    type Error = chrono::ParseError;

    fn try_from(t: String) -> Result<Self, Self::Error> {
        std::convert::TryFrom::try_from(&t)
    }
}

impl std::convert::TryFrom<&String> for Timestamp {
    type Error = chrono::ParseError;

    fn try_from(t: &String) -> Result<Self, Self::Error> {
        let t: &str = &t;
        std::convert::TryFrom::try_from(t)
    }
}

impl std::convert::TryFrom<&str> for Timestamp {
    type Error = chrono::ParseError;

    fn try_from(t: &str) -> Result<Self, Self::Error> {
        let t = chrono::DateTime::parse_from_rfc3339(t)?;
        let t = chrono::DateTime::from_utc(t.naive_utc(), chrono::Utc);
        Ok(t.into())
    }
}

impl From<Timestamp> for holochain_zome_types::timestamp::Timestamp {
    fn from(ts: Timestamp) -> Self {
        Self(ts.0, ts.1)
    }
}

impl From<holochain_zome_types::timestamp::Timestamp> for Timestamp {
    fn from(ts: holochain_zome_types::timestamp::Timestamp) -> Self {
        Self(ts.0, ts.1)
    }
}

const SEC: usize = std::mem::size_of::<i64>();
const NSEC: usize = std::mem::size_of::<u32>();

/// Total size in bytes of a [TimestampKey]
pub const TS_SIZE: usize = SEC + NSEC;

/// A representation of a Timestamp which can go into and out of a byte slice
/// in-place without allocation. Useful for LMDB keys.
///
/// The mapping to byte slice involves some bit shifting, and so the bytes
/// should not be directly used. However, ordering is preserved when mapping
/// to a TimestampKey, which is what allows us to use it for an LMDB key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct TimestampKey([u8; TS_SIZE]);

impl TimestampKey {
    /// Constructor based on current time
    pub fn now() -> Self {
        Timestamp::now().into()
    }
}

impl From<Timestamp> for TimestampKey {
    fn from(t: Timestamp) -> TimestampKey {
        let (sec, nsec) = (t.0, t.1);
        // We have to add 2^64, so that negative numbers become positive,
        // so that correct ordering relative to other byte arrays is maintained.
        let sec: i128 = (sec as i128) - (i64::MIN as i128);
        let sec: u64 = sec as u64;
        let mut a = [0; TS_SIZE];
        a[0..SEC].copy_from_slice(&sec.to_be_bytes());
        a[SEC..].copy_from_slice(&nsec.to_be_bytes());
        TimestampKey(a)
    }
}

impl From<TimestampKey> for Timestamp {
    fn from(k: TimestampKey) -> Timestamp {
        let sec = u64::from_be_bytes(k.0[0..SEC].try_into().unwrap());
        let nsec = u32::from_be_bytes(k.0[SEC..].try_into().unwrap());
        // Since we added 2^64 during encoding, we must subtract it during
        // decoding
        let sec: i128 = (sec as i128) + (i64::MIN as i128);
        Timestamp(sec as i64, nsec)
    }
}

impl AsRef<[u8]> for TimestampKey {
    fn as_ref(&self) -> &[u8] {
        assert_eq!(self.0.len(), 12);
        &self.0
    }
}

impl From<&[u8]> for TimestampKey {
    fn from(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), 12);
        Self(<[u8; TS_SIZE]>::try_from(bytes).unwrap())
    }
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
        assert_eq!(t.0, 1588706164);
        assert_eq!(t.1, 266431045);
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

    #[test]
    fn test_timestamp_key_roundtrips() {
        // create test timestamps
        let t1 = Timestamp(i64::MIN, u32::MIN);
        let t2 = Timestamp(i64::MIN / 4, u32::MAX);
        let t3 = Timestamp::try_from("1930-01-01T00:00:00.999999999Z").unwrap();
        let t4 = Timestamp::try_from("1970-11-11T14:34:00.000000000Z").unwrap();
        let t5 = Timestamp::try_from("2020-05-05T19:16:04.266431045Z").unwrap();
        let t6 = Timestamp(i64::MAX / 4, u32::MIN);
        let t7 = Timestamp(i64::MAX, u32::MAX);

        // build corresponding keys
        let k1 = TimestampKey::from(t1.clone());
        let k2 = TimestampKey::from(t2.clone());
        let k3 = TimestampKey::from(t3.clone());
        let k4 = TimestampKey::from(t4.clone());
        let k5 = TimestampKey::from(t5.clone());
        let k6 = TimestampKey::from(t6.clone());
        let k7 = TimestampKey::from(t7.clone());

        // test Timestamp <-> TimestampKey roundtrip
        assert_eq!(t1, Timestamp::from(k1.clone()));
        assert_eq!(t2, Timestamp::from(k2.clone()));
        assert_eq!(t3, Timestamp::from(k3.clone()));
        assert_eq!(t4, Timestamp::from(k4.clone()));
        assert_eq!(t5, Timestamp::from(k5.clone()));
        assert_eq!(t6, Timestamp::from(k6.clone()));
        assert_eq!(t7, Timestamp::from(k7.clone()));

        // test TimestampKey::as_ref() <-> TimestampKey roundtrip
        assert_eq!(k1, k1.as_ref().into());
        assert_eq!(k2, k2.as_ref().into());
        assert_eq!(k3, k3.as_ref().into());
        assert_eq!(k4, k4.as_ref().into());
        assert_eq!(k5, k5.as_ref().into());
        assert_eq!(k6, k6.as_ref().into());
        assert_eq!(k7, k7.as_ref().into());

        // test absolute ordering is preserved when mapping to TimestampKey
        assert!(k1 < k2);
        assert!(k2 < k3);
        assert!(k3 < k4);
        assert!(k4 < k5);
        assert!(k5 < k6);
        assert!(k6 < k7);
    }
}
