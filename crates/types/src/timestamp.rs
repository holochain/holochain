use std::convert::TryInto;

/// A UTC timestamp for use in Holochain's headers.
///
/// Timestamp implements `Serialize` and `Display` as rfc3339 time strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    /// Seconds since UNIX epoch UTC (midnight 1970-01-01).
    pub sec: i64,
    /// Nanoseconds in addition to above seconds.
    pub nsec: u32,
}

impl serde::Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;

        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Timestamp;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "expected rfc3339 time string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                v.try_into().map_err(|e| E::custom(e))
            }
        }

        deserializer.deserialize_str(V)
    }
}

impl Default for Timestamp {
    fn default() -> Self {
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
        Timestamp {
            sec: t.timestamp(),
            nsec: t.timestamp_subsec_nanos(),
        }
    }
}

impl From<Timestamp> for chrono::DateTime<chrono::Utc> {
    fn from(t: Timestamp) -> Self {
        std::convert::From::from(&t)
    }
}

impl From<&Timestamp> for chrono::DateTime<chrono::Utc> {
    fn from(t: &Timestamp) -> Self {
        let t = chrono::naive::NaiveDateTime::from_timestamp(t.sec, t.nsec);
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

    const TEST_TS: &'static str = "\"2020-05-05T19:16:04.266431045Z\"";

    #[test]
    fn test_timestamp_serialization() {
        let t: Timestamp = serde_json::from_str(TEST_TS).unwrap();
        assert_eq!(t.sec, 1588706164);
        assert_eq!(t.nsec, 266431045);
        assert_eq!(&TEST_TS[1..31], &t.to_string());
        let s = serde_json::to_string(&t).unwrap();
        assert_eq!(s, TEST_TS);
    }
}
