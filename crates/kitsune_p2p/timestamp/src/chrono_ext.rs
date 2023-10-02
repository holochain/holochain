use super::*;
use std::{convert::TryFrom, fmt, ops::Sub, str::FromStr};

pub(crate) type DateTime = chrono::DateTime<chrono::Utc>;

/// Display as RFC3339 Date+Time for sane value ranges (0000-9999AD).  Beyond that, format
/// as (seconds, nanoseconds) tuple (output and parsing of large +/- years is unreliable).
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ce = -(62167219200 * MM)..=(253402214400 * MM);
        if ce.contains(&self.0) {
            if let Ok(ts) = chrono::DateTime::<chrono::Utc>::try_from(self) {
                return write!(
                    f,
                    "{}",
                    ts.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
                );
            }
        }
        // Outside 0000-01-01 to 9999-12-31; Display raw value tuple, or not a valid DateTime<Utc>;
        // Display raw value tuple
        write!(f, "({}μs)", self.0)
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Timestamp({})", self)
    }
}

impl TryFrom<String> for Timestamp {
    type Error = TimestampError;

    fn try_from(t: String) -> Result<Self, Self::Error> {
        Timestamp::from_str(t.as_ref())
    }
}

impl TryFrom<&String> for Timestamp {
    type Error = TimestampError;

    fn try_from(t: &String) -> Result<Self, Self::Error> {
        Timestamp::from_str(t.as_ref())
    }
}

impl TryFrom<&str> for Timestamp {
    type Error = TimestampError;

    fn try_from(t: &str) -> Result<Self, Self::Error> {
        Timestamp::from_str(t)
    }
}

impl From<DateTime> for Timestamp {
    fn from(t: DateTime) -> Self {
        std::convert::From::from(&t)
    }
}

impl From<&DateTime> for Timestamp {
    fn from(t: &DateTime) -> Self {
        let t = t.naive_utc();
        Timestamp(t.timestamp() * MM + t.timestamp_subsec_nanos() as i64 / 1000)
    }
}

// Implementation note: There are *no* infallible conversions from a Timestamp to a DateTime.  These
// may panic in from_timestamp due to out-of-range secs or nsecs, making all code using/displaying a
// Timestamp this way dangerously fragile!  Use try_from, and handle any failures.

impl TryFrom<Timestamp> for DateTime {
    type Error = TimestampError;

    fn try_from(t: Timestamp) -> Result<Self, Self::Error> {
        std::convert::TryFrom::try_from(&t)
    }
}

impl TryFrom<&Timestamp> for DateTime {
    type Error = TimestampError;

    fn try_from(t: &Timestamp) -> Result<Self, Self::Error> {
        let (secs, nsecs) = t.as_seconds_and_nanos();
        let t = chrono::naive::NaiveDateTime::from_timestamp_opt(secs, nsecs)
            .ok_or(TimestampError::Overflow)?;
        Ok(chrono::DateTime::from_utc(t, chrono::Utc))
    }
}

impl FromStr for Timestamp {
    type Err = TimestampError;

    fn from_str(t: &str) -> Result<Self, Self::Err> {
        let t = chrono::DateTime::parse_from_rfc3339(t)?;
        let t = chrono::DateTime::from_utc(t.naive_utc(), chrono::Utc);
        Ok(t.into())
    }
}

impl Timestamp {
    /// Returns the current system time as a Timestamp.
    ///
    /// This is behind a feature because we need Timestamp to be WASM compatible, and
    /// chrono doesn't have a now() implementation for WASM.
    #[cfg(feature = "now")]
    pub fn now() -> Timestamp {
        Timestamp::from(chrono::offset::Utc::now())
    }
    /// Compute signed difference between two Timestamp, returning `None` if overflow occurred, or
    /// Some(chrono::Duration).  Produces Duration for differences of up to +/- i64::MIN/MAX
    /// microseconds.
    pub fn checked_difference_signed(&self, rhs: &Timestamp) -> Option<chrono::Duration> {
        Some(chrono::Duration::microseconds(self.0.checked_sub(rhs.0)?))
    }

    /// Add a signed chrono::Duration{ secs: i64, nanos: i32 } to a Timestamp.
    pub fn checked_add_signed(&self, rhs: &chrono::Duration) -> Option<Timestamp> {
        Some(Self(self.0.checked_add(rhs.num_microseconds()?)?))
    }

    /// Subtracts a chrono::Duration from a Timestamp
    pub fn checked_sub_signed(&self, rhs: &chrono::Duration) -> Option<Timestamp> {
        self.checked_add_signed(&-*rhs)
    }
}
/// Distance between two Timestamps as a chrono::Duration (subject to overflow).  A Timestamp
/// represents a *signed* distance from the UNIX Epoch (1970-01-01T00:00:00Z).  A chrono::Duration
/// is limited to +/- i64::MIN/MAX microseconds.
impl Sub<Timestamp> for Timestamp {
    type Output = TimestampResult<chrono::Duration>;

    fn sub(self, rhs: Timestamp) -> Self::Output {
        self.checked_difference_signed(&rhs)
            .ok_or(TimestampError::Overflow)
    }
}
