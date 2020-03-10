//! This crate defines traits for the two Conductor APIs:
//! - [CellConductorApiT], the internal API used in Cells, and
//! - [ExternalConductorApiT], the external API used in e.g. Interfaces

mod error;
pub mod external;
mod internal;

pub use error::{ConductorApiError, ConductorApiResult};
pub use internal::CellConductorApiT;
