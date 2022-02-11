use crate::{DateTime, Timestamp};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

/// A human-readable timestamp which is represented/serialized as an RFC3339
/// when possible, and a microsecond integer count otherwise.
/// Both representations can be deserialized to this type.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum HumanTimestamp {
    Micros(Timestamp),
    RFC3339(DateTime),
}

impl From<Timestamp> for HumanTimestamp {
    fn from(t: Timestamp) -> Self {
        DateTime::try_from(t)
            .map(Self::RFC3339)
            .unwrap_or_else(|_| Self::Micros(t))
    }
}

impl From<DateTime> for HumanTimestamp {
    fn from(t: DateTime) -> Self {
        Self::RFC3339(t)
    }
}

impl From<HumanTimestamp> for Timestamp {
    fn from(h: HumanTimestamp) -> Self {
        match h {
            HumanTimestamp::Micros(t) => t,
            HumanTimestamp::RFC3339(d) => d.into(),
        }
    }
}

impl From<&HumanTimestamp> for Timestamp {
    fn from(h: &HumanTimestamp) -> Self {
        match h {
            HumanTimestamp::Micros(t) => *t,
            HumanTimestamp::RFC3339(d) => d.into(),
        }
    }
}

impl PartialEq for HumanTimestamp {
    fn eq(&self, other: &Self) -> bool {
        Timestamp::from(self) == Timestamp::from(other)
    }
}

impl Eq for HumanTimestamp {}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use holochain_serialized_bytes::{holochain_serial, SerializedBytes};

    holochain_serial!(HumanTimestamp);

    #[test]
    fn human_timestamp_conversions() {
        let show = |v| format!("{:?}", v);
        let s = "2022-02-11T23:05:19.470323Z";
        let t = Timestamp::from_str(s).unwrap();
        let h = HumanTimestamp::from(t);
        let sb = SerializedBytes::try_from(h).unwrap();
        let ser = show(&sb);
        assert_eq!(ser, format!("\"{}\"", s));
        let h2 = HumanTimestamp::try_from(sb).unwrap();
        let t2 = Timestamp::from(h2);
        assert_eq!(t, t2);
    }
}
