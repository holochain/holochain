use crate::ZomeName;
use std::time::Duration;

/// Tick the scheduler every this many millis.
pub const SCHEDULER_INTERVAL_MILLIS: u64 = 10000;

/// Expire persisted schedules after this long.
pub const PERSISTED_TIMEOUT_MILLIS: i64 = 20000;

/// Scheduling errors.
#[derive(Debug, thiserror::Error)]
pub enum ScheduleError {
    /// Something went wrong, probably parsing a cron tab.
    #[error("{0}")]
    Cron(String),
    /// Timestamp issues.
    #[error(transparent)]
    Timestamp(crate::timestamp::TimestampError),
}

/// Defines either a persisted or ephemeral schedule for a schedule function.
/// Persisted schedules survive a conductor reboot, ephemeral will not.
/// Persisted schedules continue beyond irrecoverable errors, ephemeral do not.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Schedule {
    /// Persisted schedules are defined by a crontab syntax string.
    Persisted(String),
    /// Ephemeral schedules are defined by a Duration.
    Ephemeral(Duration),
}

impl From<String> for Schedule {
    fn from(cron: String) -> Self {
        Self::Persisted(cron)
    }
}

impl From<Duration> for Schedule {
    fn from(timeout: Duration) -> Self {
        Self::Ephemeral(timeout)
    }
}

/// A fully qualified scheduled function.
#[derive(Debug, Clone)]
pub struct ScheduledFn(ZomeName, String);

impl ScheduledFn {
    /// Constructor.
    pub fn new(zome_name: ZomeName, fn_name: String) -> Self {
        Self(zome_name, fn_name)
    }

    /// ZomeName accessor.
    pub fn zome_name(&self) -> &ZomeName {
        &self.0
    }

    /// Function name accessor.
    pub fn fn_name(&self) -> &String {
        &self.1
    }
}
