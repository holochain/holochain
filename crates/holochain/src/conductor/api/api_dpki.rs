//! The DpkiApi allows access to the DPKI service of a conductor.

use std::sync::Arc;

use holochain_conductor_services::DpkiService;

/// Alias for an optional DPKI conductor service.
pub type DpkiApi = Option<Arc<DpkiService>>;
