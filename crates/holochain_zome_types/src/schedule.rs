use std::time::Duration;

/// Defines either a persisted or ephemeral schedule for a schedule function.
/// Persisted schedules survive a conductor reboot, ephemeral will not.
/// Persisted schedules continue beyond irrecoverable errors, ephemeral do not.
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
