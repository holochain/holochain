//! Conductor Services
//!
//! The conductor expects to be able to interface with some arbitrarily defined "services" whose
//! implementation details we don't know or care about. We want well-defined interfaces for these
//! services such that a third party could write their own.

use std::sync::Arc;

mod deepkey_service;
pub use deepkey_service::*;

/// The set of all Conductor Services available to the conductor
#[derive(Clone)]
pub struct ConductorServices {
    /// The Deepkey service
    pub deepkey: Arc<dyn DeepkeyService>,
}

impl Default for ConductorServices {
    fn default() -> Self {
        Self {
            deepkey: todo!("instantiate deepkey service"),
        }
    }
}
