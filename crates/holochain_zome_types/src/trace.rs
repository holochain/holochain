//! Types related to the `debug` host function

use holochain_serialized_bytes::prelude::*;

/// Maps directly to the tracing Levels but here to define the interface.
/// @see https://docs.rs/tracing-core/0.1.17/tracing_core/struct.Level.html
#[derive(PartialEq, serde::Serialize, serde::Deserialize, Debug, Clone)]
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

impl From<&tracing::Level> for Level {
    fn from (level: &tracing::Level) -> Self {
        match level {
            &tracing::Level::ERROR => Self::ERROR,
            &tracing::Level::WARN => Self::WARN,
            &tracing::Level::INFO => Self::INFO,
            &tracing::Level::DEBUG => Self::DEBUG,
            &tracing::Level::TRACE => Self::TRACE,
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

/// Returns a [`TraceMsg`][] combining the message passed `trace_msg!` with
/// the source code location in which it's called.
///
/// # Examples
///
/// Basic usage
///
/// ```rust
/// // Due to doc-test weirdness, this comment is technically on line 4.
/// let message: TraceMsg = trace_msg!("info: operation complete");
///
/// assert_eq!(message.msg(), "info: operation complete");
/// assert_eq!(message.file(), "src/debug.rs");
/// assert_eq!(message.line(), 5);
/// # use holochain_zome_types::{trace::TraceMsg, trace_msg};
/// ```
///
/// Advanced formatting
///
/// ```rust
/// let operation = "frobnicate";
///
/// // Due to doc-test weirdness, this comment is technically on line 6.
/// let message: TraceMsg = trace_msg!(
///     "info: operation complete: {}",
///     operation
/// );
///
/// assert_eq!(message.msg(), "info: operation complete: frobnicate");
/// assert_eq!(message.file(), "src/debug.rs");
/// assert_eq!(message.line(), 7);
/// # use holochain_zome_types::{trace::TraceMsg, trace_msg};
/// ```
///
/// [`TraceMsg`]: struct.TraceMsg.html
#[macro_export]
macro_rules! trace_msg {
    ( $level:expr, $msg:expr ) => {
        holochain_zome_types::trace_msg!($level, "{}", $msg);
    };
    ( $level:expr, $msg:expr, $($tail:expr),* ) => {{
        $crate::trace::TraceMsg{
            msg: format!(
                "{}:{}:{} {}", 
                module_path!(), 
                file!(), 
                line!(), 
                format!($msg, $($tail),*),
            ),
            level: $level,
        }
    }};
}
