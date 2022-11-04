//! A Conductor manages interactions between its contained [Cell]s, as well as
//! interactions with the outside world. It is primarily a mediator of messages.
//!
//! The Conductor exposes two types of external interfaces:
//! - App interface: used by Holochain app UIs to drive the behavior of Cells,
//! - Admin interface: used to modify the Conductor itself, including adding and removing Cells
//!
//! It also exposes an internal interface to Cells themselves, allowing Cells
//! to call zome functions on other Cells, as well as to send Signals to the
//! outside world

#![deny(missing_docs)]

// TODO: clean up allow(missing_docs) once parent is fully documented

pub mod api;
mod cell;
#[cfg(feature = "chc")]
pub mod chc;
#[allow(clippy::module_inception)]
#[allow(missing_docs)]
pub mod conductor;
#[allow(missing_docs)]
pub mod config;
pub mod entry_def_store;
#[allow(missing_docs)]
pub mod error;
pub mod interactive;
pub mod interface;
pub mod kitsune_host_impl;
pub mod manager;
pub mod p2p_agent_store;
pub mod paths;
#[allow(missing_docs)]
pub mod ribosome_store;
pub mod space;
pub mod state;

pub use cell::error::CellError;
pub use cell::Cell;
pub use conductor::Conductor;
pub use conductor::ConductorBuilder;
pub use conductor::ConductorHandle;
pub use conductor::{full_integration_dump, integration_dump};

#[cfg(test)]
mod tests;
