//! How holochain conductor makes use of rust tracing/logging.
//!
//! ### `RUST_LOG` Environment Variable
//!
//! See the [tracing-subscriber](https://crates.io/crates/tracing-subscriber)
//! crate's documentation for usage of the EnvFilter. Essentially you can
//! specify tracing/logging levels via the environment variable RUST_LOG.
//!
//! Examples:
//! - `RUST_LOG=warn` - only print warn and error levels
//! - `RUST_LOG=trace` - print ALL very verbose tracing logs
//! - `RUST_LOG=off,NETAUDIT=trace` - print ONLY the "NETAUDIT" target tracing
//!
//! ### Conductor Config `tracing_override` Field
//!
//! If, for some reason you cannot specify an environment variable, you
//! can also set the tracing level via the conductor config
//! `tracing_override` field.
//!
//! ### `NETAUDIT` Target
//!
//! The special `NETAUDIT` target is a cross-crate tracing target for
//! getting a handle on what conductor is doing with remote communications
//! under the hood. Specific traces are output in:
//! - sbd-client
//! - tx5-connection
//! - tx5
//! - kitsune_p2p
//! - and holochain itself
//!
//! Where appropriate, try to set some standardized properties on the trace:
//!
//! - `m` - module or crate in which the trace is defined
//! - `t` - type or additional internal context for making sense of the trace
//! - `a` - action or event described by the trace
//!
//! E.g.: `m="tx5" t="signal" a="connected"`
//!
//! To see the output, use a tracing configuration such as
//! `RUST_LOG=off,NETAUDIT=trace`.
