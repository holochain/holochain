/// A UTC timestamp for use in Holochain's headers.
///
/// Timestamp implements `Serialize` and `Display` as rfc3339 time strings.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct Timestamp(
    /// Seconds since UNIX epoch UTC (midnight 1970-01-01).
    pub i64,
    /// Nanoseconds in addition to above seconds.
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

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TS: &'static str = "2020-05-05T19:16:04.266431045Z";
    const TEST_EN: &'static [u8] = b"\x92\xce\x5e\xb1\xbb\x74\xce\x0f\xe1\x6a\x45";

    #[test]
    fn test_timestamp_serialization() {
        use holochain_serialized_bytes::prelude::*;
        use std::convert::TryInto;

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
}
