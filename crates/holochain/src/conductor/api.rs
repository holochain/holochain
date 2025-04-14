#![deny(missing_docs)]

//! Defines the three Conductor APIs by which other code can communicate
//! with a [`Conductor`](super::Conductor):
//!
//! - [`CellConductorApi`], for Cells to communicate with their Conductor
//! - [`AppInterfaceApi`], for external UIs to e.g. call zome functions on a Conductor
//! - [`AdminInterfaceApi`], for external processes to e.g. modify ConductorState
//!
//! Each type of API uses a [`ConductorHandle`](super::ConductorHandle) as its exclusive means of conductor access

mod api_cell;
mod api_external;
#[allow(missing_docs)]
pub mod error;

pub use api_cell::*;
pub use api_external::*;
