pub mod api;
mod cell;
#[allow(clippy::module_inception)]
mod conductor;
pub mod config;
pub mod error;
pub mod interactive;
pub mod interface;
pub mod manager;
pub mod paths;
pub mod state;

pub use cell::Cell;
pub use conductor::{Conductor, ConductorHandle, GhostConductor};

//FIXME should this be here?
// #[cfg(test)]
// mod test_fixtures;
