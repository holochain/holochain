//! # Timestamp

#[allow(missing_docs)]
mod error;

use std::{
    convert::TryFrom,
    fmt,
    ops::{Add, Sub},
    str::FromStr,
};

use crate::prelude::*;

pub use error::{TimestampError, TimestampResult};

/// A UTC timestamp for use in Holochain's headers.  It is assumed to be untrustworthy: it may
/// contain times offset from the UNIX epoch with the full +/- i64 range.  Most of these times are
/// *not* representable by a chrono::DateTime<Utc> (which limits itself to a +/- i32 offset in days
/// from Jan 1, 0AD and from 1970AD).  Also, most differences between two Timestamps are *not*
/// representable by either a chrono::Duration (which limits itself to +/- i64 milliseconds), *nor*
/// by core::time::Duration (which limits itself to +'ve u64 seconds).  Many constructions of these
/// chrono and core::time types will panic!, so painful measures must be taken to avoid this outcome
/// -- it is not acceptable for our core Holochain algorithms to panic when accessing DHT Header
/// information committed by other random Holochain nodes!
///
/// Timestamp implements `Serialize` and `Display` as rfc3339 time strings (if possible).
/// - Field 0: i64 - Seconds since UNIX epoch UTC (midnight 1970-01-01).
/// - Field 1: u32 - Nanoseconds in addition to above seconds, always in positive direction.
///
/// Supports +/- chrono::Duration directly.  There is no Timestamp::now() method, since this is not
/// supported by WASM; however, holochain_types provides a timestamp::now() method.
///
/// Create a new Timestamp instance from the supplied secs/nsecs.  Note that we can easily create a
/// Timestamp that cannot be converted to a valid DateTime<Utc> (ie. by supplying 86,400-second days
/// beyond range of +/- i32 offset from 0AD or 1970AD, nsecs beyond 1e9, etc.; see its code.)
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, SerializedBytes,
)]
pub struct Timestamp(
    pub i64, // seconds from UNIX Epoch, positive or negative
    pub u32, // nanoseconds, always a positive offset
);

/// Display as RFC3339 Date+Time for sane value ranges (0000-9999AD).  Beyond that, format
/// as (seconds, nanoseconds) tuple (output and parsing of large +/- years is unreliable).
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
	let ce = -62167219200_i64..=253402214400_i64;
	if ce.contains(&self.0) {
	    if let Ok(ts) = chrono::DateTime::<chrono::Utc>::try_from(self) {
		return write!(f, "{}", ts.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true));
	    }
	}
	// Outside 0000-01-01 to 9999-12-31; Display raw value tuple, or not a valid DateTime<Utc>;
	// Display raw value tuple
	write!(f, "({},{})", self.0, self.1)
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Timestamp({})", self)
    }
}

/// Infallible conversions into a Timestamp.  The only infallible ways to create a Timestamp are
/// `from` a Unix timestamp, or `normalize` with a timestamp and nanoseconds, or converting from
/// a DateTime<Utc>.
impl From<i64> for Timestamp {
    fn from(secs: i64) -> Self {
        Self(secs, 0)
    }
}

impl From<i32> for Timestamp {
    fn from(secs: i32) -> Self {
        Self(secs.into(), 0)
    }
}

impl From<u32> for Timestamp {
    fn from(secs: u32) -> Self {
        Self(secs.into(), 0)
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
        let t = chrono::naive::NaiveDateTime::from_timestamp_opt(t.0, t.1)
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
	Ok(self.checked_add(&rhs.into())
	   .ok_or(TimestampError::Overflow)?)
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
        Ok(self.checked_sub(&rhs.into())
           .ok_or(TimestampError::Overflow)?)
    }
}

impl<D: Into<core::time::Duration>> Sub<D> for &Timestamp {
    type Output = TimestampResult<Timestamp>;

    fn sub(self, rhs: D) -> Self::Output {
        self.to_owned() - rhs
    }
}

macro_rules! try_opt {
    ($e:expr) => (match $e { Some(v) => v, None => return None })
}

impl Timestamp {
    /// Construct a normalized Timestamp from the given secs/nanos.  Allows a full, signed range of
    /// seconds and/or nanoseconds; produces a Timestamp with a properly signed i64 seconds, and an
    /// always positive-offset u32 nanoseconds.  Differs from typical `new` implementation in that
    /// it returns an Option<Timestamp>.
    /// 
    /// ```
    /// use holochain_zome_types::prelude::*;
    /// assert_eq!( Timestamp::normalize( 0, -1 ).unwrap(), Timestamp( -1, 999_999_999 ))
    /// ```
    pub fn normalize(secs: i64, nanos: i64) -> Option<Timestamp> {
	// eg. -1_234_567_890 / 1_000_000_000 == -1
	let seconds = try_opt!(secs.checked_add(nanos / 1_000_000_000));
	// eg. -1_234_567_890 % 1_000_000_000 == -235_567_890
	let nanos = nanos % 1_000_000_000; // in range (-999_999_999,999_999_999)
	let ts = if nanos < 0 {
	    let seconds = try_opt!(secs.checked_sub(1));
	    let nanos = try_opt!(nanos.checked_add(1_000_000_000));
	    let nanos = try_opt!(u32::try_from(nanos).ok()); // now in range: (0,999_999_999)
	    Timestamp(seconds, nanos)
	} else {
	    let nanos = try_opt!(u32::try_from(nanos).ok());
	    Timestamp(seconds, nanos)
	};
	Some(ts)
    }

    /// Compute signed difference between two Timestamp, returning `None` if overflow occurred, or
    /// Some(chrono::Duration).  Produces Duration for differences of up to +/- i64::MIN/MAX
    /// milliseconds (the full range of a signed chrono::Duration).  Note that, surprisingly, there
    /// is almost no way to create a chrono::Duration that does not (directly or indirectly) have
    /// the possibility of panic!  One of the few paths is Duration::milliseconds() and smaller (all
    /// larger use Duration::seconds, which may directly panic!), followed by a
    /// Duration::checked_add for the nanoseconds.
    pub fn checked_difference_signed(&self, rhs: &Timestamp) -> Option<chrono::Duration> {
	let dif_secs = try_opt!(self.0
				.checked_sub(rhs.0));
	let dif_nano = try_opt!(i64::from(self.1)
				.checked_sub(i64::from(rhs.1)));
	let dif = try_opt!(Timestamp::normalize(dif_secs, dif_nano));
	let dur_milli = chrono::Duration::milliseconds(try_opt!(dif.0.checked_mul(1_000)));
	let dur_nanos = chrono::Duration::nanoseconds(dif.1.into()); // u32 -> i64, no overflow possible
	let dur = try_opt!(dur_milli.checked_add(&dur_nanos));
	Some(dur)
    }

    /// Add a signed chrono::Duration{ secs: i64, nanos: i32 } (-'ve nanos are invalid) to a
    /// Timestamp( i64, u32 ).  May overflow.  Unfortunately, there is *no way* in the provided API
    /// to actually obtain the raw { secs, nanos }, nor their component parts without overflow!  The
    /// closest is to obtain the millis, subtract them out and obtain the residual nanoseconds...
    /// 
    /// ```
    /// use holochain_zome_types::prelude::*;
    ///
    /// assert_eq!( Timestamp::normalize( 0, 1 ).unwrap()
    ///                 .checked_sub_signed(&chrono::Duration::nanoseconds(2)),
    ///             Some(Timestamp( -1, 999_999_999 )));
    /// //assert_eq!((Timestamp::normalize( 0, 1 ).unwrap()
    /// //            - chrono::Duration::nanoseconds(2)),
    /// //            Some(Timestamp( -1, 999_999_999 )));
    /// ```
    pub fn checked_add_signed(&self, rhs: &chrono::Duration) -> Option<Timestamp> {
	let dur_millis: i64 = rhs.num_milliseconds();
	let rhs_remains = try_opt!(rhs.checked_sub(&chrono::Duration::milliseconds(dur_millis)));
	let dur_nanos: i64 = try_opt!(rhs_remains.num_nanoseconds()) + (dur_millis % 1_000) * 1_000_000;
	let dur_seconds: i64 = dur_millis / 1_000;
	let seconds: i64 = try_opt!(self.0.checked_add(dur_seconds));
	let nanos: i64 = try_opt!(i64::from(self.1).checked_add(dur_nanos));
	Some(try_opt!(Timestamp::normalize(seconds, nanos)))
    }

    /// Subtracts a chrono::Duration from a Timestamp
    /// ```
    /// ```
    pub fn checked_sub_signed(&self, rhs: &chrono::Duration) -> Option<Timestamp> {
	self.checked_add_signed(&-*rhs)
    }

    /// Add unsigned core::time::Duration{ secs: u64, nanos: u32 } to a Timestamp.  See:
    /// https://doc.rust-lang.org/src/core/time.rs.html#53-56
    /// ```
    /// use holochain_zome_types::prelude::*;
    ///
    /// assert_eq!( Timestamp::normalize( 0, -3 ).unwrap()
    ///                 .checked_add(&core::time::Duration::from_nanos(2)),
    ///             Some(Timestamp( -1, 999_999_999 )));
    /// assert_eq!( Timestamp::normalize( 0, 0 ).unwrap()
    ///                 .checked_add(&core::time::Duration::from_secs(2_u64.pow(32)-1)),
    ///             Some(Timestamp( 2_i64.pow(32)-1, 0 )));
    /// assert_eq!( Timestamp::normalize( 0, 0 ).unwrap()
    ///                 .checked_add(&core::time::Duration::from_secs(2_u64.pow(63)-1)),
    ///             Some(Timestamp( (2_u64.pow(63)-1) as i64, 0 )));
    /// assert_eq!( Timestamp::normalize( 0, 0 ).unwrap()
    ///                 .checked_add(&core::time::Duration::from_secs(2_u64.pow(63))),
    ///             None);
    /// ```
    pub fn checked_add(&self, rhs: &core::time::Duration) -> Option<Timestamp> {
	let dur_seconds: i64 = try_opt!(i64::try_from(rhs.as_secs()).ok());
	let dur_nanos: i64 = i64::from(rhs.subsec_nanos());
	let seconds: i64 = try_opt!(self.0.checked_add(dur_seconds));
	let nanos: i64 = try_opt!(i64::from(self.1).checked_add(dur_nanos));
	Some(try_opt!(Timestamp::normalize(seconds, nanos)))
    }

    /// Sub unsigned core::time::Duration{ secs: u64, nanos: u32 } from a Timestamp.
    /// ```
    /// use holochain_zome_types::prelude::*;
    ///
    /// assert_eq!( Timestamp::normalize( 0, 1 ).unwrap()
    ///                 .checked_sub(&core::time::Duration::from_nanos(2)),
    ///             Some(Timestamp( -1, 999_999_999 )));
    /// assert_eq!((Timestamp::normalize( 0, 1 ).unwrap()
    ///             - core::time::Duration::from_nanos(2)),
    ///             Ok(Timestamp( -1, 999_999_999 )));
    /// assert_eq!( Timestamp::normalize( 550, 5_500_000_000 ).unwrap()
    ///                 .checked_sub(&core::time::Duration::from_nanos(2)),
    ///             Some(Timestamp( 555, 499_999_998 )));
    /// ```
    pub fn checked_sub(&self, rhs: &core::time::Duration) -> Option<Timestamp> {
	let dur_seconds: i64 = try_opt!(i64::try_from(rhs.as_secs()).ok());
	let dur_nanos: i64 = i64::from(rhs.subsec_nanos());
	let seconds: i64 = try_opt!(self.0.checked_sub(dur_seconds));
	let nanos: i64 = try_opt!(i64::from(self.1).checked_sub(dur_nanos));
	Some(try_opt!(Timestamp::normalize(seconds, nanos)))
    }
}

/// Distance between two Timestamps as a chrono::Duration (subject to overflow).  A Timestamp
/// represents a *signed* distance from the UNIX Epoch (1970-01-01T00:00:00Z).  A chrono::Duration
/// is limited to +/- i64::MIN/MAX milliseconds.
impl Sub<Timestamp> for Timestamp {
    type Output = TimestampResult<chrono::Duration>;

    fn sub(self, rhs: Timestamp) -> Self::Output {
	Ok(self.checked_difference_signed(&rhs)
	   .ok_or(TimestampError::Overflow)?)
    }
}


#[cfg(test)]
pub mod tests {

    use super::*;

    #[test]
    fn timestamp_distance() {
	// Obtaining an ordering of timestamps and their difference / distance is subtle and error
	// prone.  It is easy to get panics when converting Timestamp to chrono::Datetime<Utc> and
	// chrono::Duration, both of which have strict range limits.  Since we cannot generally
	// trust code that produces Timestamps, it has no intrinsic range limits.
	let t1 = Timestamp( (2_i64.pow(31)+1)*86_400, 1_000_000_000 ); // invalid secs for DateTime
	let d1: TimestampResult<chrono::DateTime<chrono::Utc>> = t1.try_into();
	assert_eq!( d1,  Err(TimestampError::Overflow));

	let t2 = Timestamp( 0, 0 ) + core::time::Duration::new(0,1);
	assert_eq!( t2,  Ok(Timestamp( 0, 1 )));
    }
}
