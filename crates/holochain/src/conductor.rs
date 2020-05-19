// TODO: clean up deny's once parent is fully documented
#[deny(missing_docs)]
pub mod api;
mod cell;
pub mod compat;
#[allow(clippy::module_inception)]
mod conductor;
#[deny(missing_docs)]
pub mod config;
pub mod dna_store;
pub mod error;
#[deny(missing_docs)]
pub mod handle;
#[deny(missing_docs)]
pub mod interactive;
pub mod interface;
#[deny(missing_docs)]
pub mod manager;
#[deny(missing_docs)]
pub mod paths;
pub mod state;

pub use cell::{error::CellError, Cell};
pub use conductor::{Conductor, ConductorBuilder};
pub use handle::ConductorHandle;
