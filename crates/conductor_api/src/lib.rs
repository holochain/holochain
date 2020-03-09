//! This crate defines traits for the two Conductor APIs:
//! - [CellConductorApiT], the internal API used in Cells, and
//! - [ExternalConductorApiT], the external API used in e.g. Interfaces

mod cell;
mod conductor;
mod error;
pub mod external;
mod internal;

pub use cell::ApiCellT;
pub use conductor::ApiConductorT;
pub use error::{ConductorApiError, ConductorApiResult};
pub use external::ExternalConductorApiT;
pub use internal::CellConductorApiT;
