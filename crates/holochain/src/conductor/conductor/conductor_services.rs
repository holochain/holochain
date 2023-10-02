//! Conductor Services
//!
//! The conductor expects to be able to interface with some arbitrarily defined "services" whose
//! implementation details we don't know or care about. We want well-defined interfaces for these
//! services such that a third party could write their own.

use std::{collections::HashSet, sync::Arc};

mod dpki_service;
pub use dpki_service::*;

mod app_store_service;
pub use app_store_service::*;

use super::ConductorHandle;
use holochain_zome_types::CellId;

/// The set of all Conductor Services available to the conductor
#[derive(Clone)]
pub struct ConductorServices {
    /// The DPKI service
    pub dpki: Arc<dyn DpkiService>,
    /// The AppStore service
    pub app_store: Arc<dyn AppStoreService>,
}

impl ConductorServices {
    /// Construct services from the default built-in implementations
    pub fn builtin(conductor: ConductorHandle, cell_ids: ConductorServiceCells) -> Self {
        Self {
            dpki: DeepkeyBuiltin::new(conductor.clone(), cell_ids.dpki),
            app_store: AppStoreBuiltin::new(conductor, cell_ids.app_store),
        }
    }

    /// Get the list of any CellIds which may be protected due to being in use by ConductorServices
    pub fn protected_cell_ids(&self) -> HashSet<&CellId> {
        self.dpki
            .cell_ids()
            .union(&self.app_store.cell_ids())
            .copied()
            .collect()
    }
}

/// Initialized for ConductorService: just the CellIds that are used for each service
pub struct ConductorServiceCells {
    /// The CellId to use for DPKI
    pub dpki: CellId,
    /// The CellId to use for the AppStore
    pub app_store: CellId,
}
