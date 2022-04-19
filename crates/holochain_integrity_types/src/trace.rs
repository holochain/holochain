//! Types related to the `debug` host function

use holochain_serialized_bytes::prelude::*;

/// Maps directly to the tracing Levels but here to define the interface.
/// See https://docs.rs/tracing-core/0.1.17/tracing_core/struct.Level.html
#[derive(PartialEq, serde::Serialize, serde::Deserialize, Debug, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub enum Level {
    /// Error.
    ERROR,
    /// Warning.
    WARN,
    /// Info.
    INFO,
    /// Debug.
    DEBUG,
    /// Trace.
    TRACE,
}

#[cfg(feature = "tracing")]
impl From<&tracing::Level> for Level {
    fn from(level: &tracing::Level) -> Self {
        match *level {
            tracing::Level::ERROR => Self::ERROR,
            tracing::Level::WARN => Self::WARN,
            tracing::Level::INFO => Self::INFO,
            tracing::Level::DEBUG => Self::DEBUG,
            tracing::Level::TRACE => Self::TRACE,
        }
    }
}

/// Representation of message to be logged via the `debug` host function
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TraceMsg {
    /// A formatted string to be forwarded to `tracing` on the host side.
    ///
    /// The host will provide:
    /// - Timestamps
    /// - ANSI coloured levels
    ///
    /// The guest should provide:
    /// - Useful message
    /// - Line numbers etc.
    pub msg: String,
    /// Severity level for the message.
    pub level: Level,
}
