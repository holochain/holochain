//! A channel-based implementation of network connections, for direct manipulation
//! of the medium of message exchange, used during testing

#![warn(missing_docs)]

pub mod switchboard;
pub mod switchboard_evt_handler;

#[cfg(test)]
mod tests;
