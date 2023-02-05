//! A microsecond-precision UTC timestamp for use in Holochain's actions.
#![deny(missing_docs)]

#[allow(missing_docs)]
mod error;
#[cfg(feature = "chrono")]
mod human;

#[cfg(feature = "chrono")]
pub use human::*;

use core::ops::{Add, Sub};
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};

pub use crate::error::{TimestampError, TimestampResult};

#[cfg(feature = "chrono")]
pub(crate) use chrono_ext::*;

#[cfg(feature = "chrono")]
mod chrono_ext;

/// One million
pub const MM: i64 = 1_000_000;

/// A microsecond-precision UTC timestamp for use in Holochain's actions.
///
/// It is assumed to be untrustworthy:
/// it may contain times offset from the UNIX epoch with the full +/- i64 range.
/// Most of these times are *not* representable by a `chrono::DateTime<Utc>`
/// (which limits itself to a +/- i32 offset in days from Jan 1, 0AD and from 1970AD).
///
/// Also, most differences between two Timestamps are *not*
/// representable by either a `chrono::Duration` (which limits itself to +/- i64 microseconds), *nor*
/// by `core::time::Duration` (which limits itself to +'ve u64 seconds).  Many constructions of these
/// chrono and core::time types will panic!, so painful measures must be taken to avoid this outcome
/// -- it is not acceptable for our core Holochain algorithms to panic when accessing DHT Action
/// information committed by other random Holochain nodes!
///
/// Timestamp implements `Serialize` and `Display` as rfc3339 time strings (if possible).
///
/// Supports +/- `chrono::Duration` directly.  There is no `Timestamp::now()` method, since this is not
/// supported by WASM; however, `holochain_types` provides a `Timestamp::now()` method.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(not(feature = "chrono"), derive(Debug))]
pub struct Timestamp(
    /// Microseconds from UNIX Epoch, positive or negative
    pub i64,
);

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
    /// The Timestamp corresponding to the UNIX epoch
    pub const ZERO: Timestamp = Timestamp(0);
    /// The smallest possible Timestamp
    pub const MIN: Timestamp = Timestamp(i64::MIN);
    /// The largest possible Timestamp
    pub const MAX: Timestamp = Timestamp(i64::MAX);
    /// Jan 1, 2022, 12:00:00 AM UTC
    pub const HOLOCHAIN_EPOCH: Timestamp = Timestamp(1640995200000000);

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

    /// Add unsigned core::time::Duration{ secs: u64, nanos: u32 } to a Timestamp.  See:
    /// <https://doc.rust-lang.org/src/core/time.rs.html#53-56>
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

    /// Create a [`Timestamp`] from a [`core::time::Duration`] saturating at i64::MAX.
    pub fn saturating_from_dur(duration: &core::time::Duration) -> Self {
        Timestamp(std::cmp::min(duration.as_micros(), i64::MAX as u128) as i64)
    }
}

impl TryFrom<core::time::Duration> for Timestamp {
    type Error = error::TimestampError;

    fn try_from(value: core::time::Duration) -> Result<Self, Self::Error> {
        Ok(Timestamp(
            value
                .as_micros()
                .try_into()
                .map_err(|_| error::TimestampError::Overflow)?,
        ))
    }
}

#[cfg(feature = "rusqlite")]
impl rusqlite::ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            self.0.into(),
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
            let micros = t.clone().as_micros();
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
