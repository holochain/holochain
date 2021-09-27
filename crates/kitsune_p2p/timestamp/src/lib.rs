//! A microsecond-precision UTC timestamp for use in Holochain's headers.

#[allow(missing_docs)]
mod error;

use std::{
    convert::TryFrom,
    fmt,
    ops::{Add, Sub},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

pub use crate::error::{TimestampError, TimestampResult};

/// One million
pub const MM: i64 = 1_000_000;

/// A microsecond-precision UTC timestamp for use in Holochain's headers.
///
/// It is assumed to be untrustworthy:
/// it may contain times offset from the UNIX epoch with the full +/- i64 range.
/// Most of these times are *not* representable by a chrono::DateTime<Utc>
/// (which limits itself to a +/- i32 offset in days from Jan 1, 0AD and from 1970AD).
///
/// Also, most differences between two Timestamps are *not*
/// representable by either a chrono::Duration (which limits itself to +/- i64 microseconds), *nor*
/// by core::time::Duration (which limits itself to +'ve u64 seconds).  Many constructions of these
/// chrono and core::time types will panic!, so painful measures must be taken to avoid this outcome
/// -- it is not acceptable for our core Holochain algorithms to panic when accessing DHT Header
/// information committed by other random Holochain nodes!
///
/// Timestamp implements `Serialize` and `Display` as rfc3339 time strings (if possible).
///
/// Supports +/- chrono::Duration directly.  There is no Timestamp::now() method, since this is not
/// supported by WASM; however, holochain_types provides a Timestamp::now() method.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Timestamp(
    i64, // microseconds from UNIX Epoch, positive or negative
);

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

impl From<chrono::DateTime<chrono::Utc>> for Timestamp {
    fn from(t: chrono::DateTime<chrono::Utc>) -> Self {
        std::convert::From::from(&t)
    }
}

impl From<&chrono::DateTime<chrono::Utc>> for Timestamp {
    fn from(t: &chrono::DateTime<chrono::Utc>) -> Self {
        let t = t.naive_utc();
        Timestamp(t.timestamp() * MM + t.timestamp_subsec_nanos() as i64 / 1000)
    }
}

// Implementation note: There are *no* infallible conversions from a Timestamp to a DateTime.  These
// may panic in from_timestamp due to out-of-range secs or nsecs, making all code using/displaying a
// Timestamp this way dangerously fragile!  Use try_from, and handle any failures.

impl TryFrom<Timestamp> for chrono::DateTime<chrono::Utc> {
    type Error = TimestampError;

    fn try_from(t: Timestamp) -> Result<Self, Self::Error> {
        std::convert::TryFrom::try_from(&t)
    }
}

impl TryFrom<&Timestamp> for chrono::DateTime<chrono::Utc> {
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

/// Timestamp +/- Into<core::time::Duration>: Anything that can be converted into a
/// core::time::Duration can be used as an overflow-checked offset (unsigned) for a Timestamp.  A
/// core::time::Duration allows only +'ve offsets
impl<D: Into<core::time::Duration>> Add<D> for Timestamp {
    type Output = TimestampResult<Timestamp>;

    fn add(self, rhs: D) -> Self::Output {
        self.checked_add(&rhs.into())
            .ok_or(TimestampError::Overflow)
    }
}

impl<D: Into<core::time::Duration>> Add<D> for &Timestamp {
    type Output = TimestampResult<Timestamp>;

    fn add(self, rhs: D) -> Self::Output {
        self.to_owned() + rhs
    }
}

/// Timestamp - core::time::Duration.
impl<D: Into<core::time::Duration>> Sub<D> for Timestamp {
    type Output = TimestampResult<Timestamp>;

    fn sub(self, rhs: D) -> Self::Output {
        self.checked_sub(&rhs.into())
            .ok_or(TimestampError::Overflow)
    }
}

impl<D: Into<core::time::Duration>> Sub<D> for &Timestamp {
    type Output = TimestampResult<Timestamp>;

    fn sub(self, rhs: D) -> Self::Output {
        self.to_owned() - rhs
    }
}

impl Timestamp {
    /// The smallest possible Timestamp
    pub const MIN: Timestamp = Timestamp(i64::MIN);
    /// The largest possible Timestamp
    pub const MAX: Timestamp = Timestamp(i64::MAX);

    /// Returns the current system time as a Timestamp.
    ///
    /// This is behind a feature because we need Timestamp to be WASM compatible, and
    /// chrono doesn't have a now() implementation for WASM.
    #[cfg(feature = "now")]
    pub fn now() -> Timestamp {
        Timestamp::from(chrono::offset::Utc::now())
    }

    /// Largest possible Timestamp.
    pub fn max() -> Timestamp {
        Timestamp(i64::MAX)
    }

    /// Construct from microseconds
    pub fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    /// Access time as microseconds since UNIX epoch
    pub fn as_micros(&self) -> i64 {
        self.0
    }

    /// Access time as milliseconds since UNIX epoch
    pub fn as_millis(&self) -> i64 {
        self.0 / 1000
    }

    /// Access seconds since UNIX epoch plus nanosecond offset
    pub fn as_seconds_and_nanos(&self) -> (i64, u32) {
        let secs = self.0 / MM;
        let nsecs = (self.0 % 1_000_000) * 1000;
        (secs, nsecs as u32)
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

    /// Add unsigned core::time::Duration{ secs: u64, nanos: u32 } to a Timestamp.  See:
    /// https://doc.rust-lang.org/src/core/time.rs.html#53-56
    pub fn checked_add(&self, rhs: &core::time::Duration) -> Option<Timestamp> {
        let micros = rhs.as_micros();
        if micros <= i64::MAX as u128 {
            Some(Self(self.0.checked_add(micros as i64)?))
        } else {
            None
        }
    }

    /// Sub unsigned core::time::Duration{ secs: u64, nanos: u32 } from a Timestamp.
    pub fn checked_sub(&self, rhs: &core::time::Duration) -> Option<Timestamp> {
        let micros = rhs.as_micros();
        if micros <= i64::MAX as u128 {
            Some(Self(self.0.checked_sub(micros as i64)?))
        } else {
            None
        }
    }

    /// Add a duration, clamping to MAX if overflow
    pub fn saturating_add(&self, rhs: &core::time::Duration) -> Timestamp {
        self.checked_add(rhs).unwrap_or(Self::MAX)
    }

    /// Subtract a duration, clamping to MIN if overflow
    pub fn saturating_sub(&self, rhs: &core::time::Duration) -> Timestamp {
        self.checked_sub(rhs).unwrap_or(Self::MIN)
    }

    /// Convert this timestamp to fit into a SQLite integer which is an i64.
    /// The value will be clamped to the valid range supported by SQLite
    pub fn into_sql_lossy(self) -> Self {
        Self(i64::clamp(self.0, -62167219200 * MM, 106751991167 * MM))
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

#[cfg(feature = "rusqlite")]
impl rusqlite::ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            self.into_sql_lossy().0.into(),
        ))
    }
}

#[cfg(feature = "rusqlite")]
impl rusqlite::types::FromSql for Timestamp {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value {
            // NB: if you have a NULLable Timestamp field in a DB, use `Option<Timestamp>`.
            //     otherwise, you'll get an InvalidType error, because we don't handle null
            //     values here.
            rusqlite::types::ValueRef::Integer(i) => Ok(Self::from_micros(i)),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    const TEST_TS: &'static str = "2020-05-05T19:16:04.266431Z";

    #[test]
    fn timestamp_distance() {
        // Obtaining an ordering of timestamps and their difference / distance is subtle and error
        // prone.  It is easy to get panics when converting Timestamp to chrono::Datetime<Utc> and
        // chrono::Duration, both of which have strict range limits.  Since we cannot generally
        // trust code that produces Timestamps, it has no intrinsic range limits.
        let t1 = Timestamp(i64::MAX); // invalid secs for DateTime
        let d1: TimestampResult<chrono::DateTime<chrono::Utc>> = t1.try_into();
        assert_eq!(d1, Err(TimestampError::Overflow));

        let t2 = Timestamp(0) + core::time::Duration::new(0, 1000);
        assert_eq!(t2, Ok(Timestamp(1)));
    }

    #[test]
    fn micros_roundtrip() {
        for t in [Timestamp(1234567890), Timestamp(987654321)] {
            let micros = t.clone().into_sql_lossy().as_micros();
            let r = Timestamp::from_micros(micros);
            assert_eq!(t.0, r.0);
            assert_eq!(t, r);
        }
    }

    #[test]
    fn test_timestamp_serialization() {
        use holochain_serialized_bytes::prelude::*;
        let t: Timestamp = TEST_TS.try_into().unwrap();
        let (secs, nsecs) = t.as_seconds_and_nanos();
        assert_eq!(secs, 1588706164);
        assert_eq!(nsecs, 266431000);
        assert_eq!(TEST_TS, &t.to_string());

        #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
        struct S(Timestamp);
        let s = S(t);
        let sb = SerializedBytes::try_from(s).unwrap();
        let s: S = sb.try_into().unwrap();
        let t = s.0;
        assert_eq!(TEST_TS, &t.to_string());
    }
}
