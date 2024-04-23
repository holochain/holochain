//! Interfaces are long-running tasks which listen for incoming messages
//! and dispatch them to the appropriate handlers within Holochain.
//! They also allow emitting responses and one-way Signals.
//!
//! Currently, the only InterfaceDriver is a Websocket-based one, whose
//! implementation can be found in the `websocket` module here.

#[allow(missing_docs)]
pub mod error;
pub mod websocket;

pub use holochain_conductor_api::config::InterfaceDriver;
