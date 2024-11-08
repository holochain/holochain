/// Kitsune2 timestamp.
///
/// Internally i64 microseconds from unix epoch.
// Legacy kitsune had a massively overcomplicated timestamp type.
// - We don't need chrono
// - We don't need human readable times
// - We don't need sqlite transformation functions
// - We DO need the ability to get the current "now" timestamp
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Construct a new timestamp of "now".
    pub fn now() -> Self {
        Self(
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("invalid system clock")
                .as_micros() as i64,
        )
    }

    /// Construct a timestamp from i64 microseconds since unix epoch.
    pub fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    /// Get the i64 microseconds since unix epoch.
    pub fn as_micros(&self) -> i64 {
        self.0
    }
}

impl std::ops::Add<std::time::Duration> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: std::time::Duration) -> Self::Output {
        Timestamp(self.0 + rhs.as_micros() as i64)
    }
}

impl From<std::time::SystemTime> for Timestamp {
    fn from(t: std::time::SystemTime) -> Self {
        Self(
            t.duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("invalid system time")
                .as_micros() as i64,
        )
    }
}

impl From<Timestamp> for std::time::SystemTime {
    fn from(t: Timestamp) -> Self {
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_micros(t.0 as u64)
    }
}
