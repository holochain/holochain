//! Conductor Services
//!
//! The conductor expects to be able to interface with some arbitrarily defined "services" whose
//! implementation details we don't know or care about. We want well-defined interfaces for these
//! services such that a third party could write their own.

use std::sync::Arc;

mod dpki_service;
pub use dpki_service::*;

mod app_store_service;
pub use app_store_service::*;

use holochain_types::prelude::*;

#[async_trait::async_trait]
pub trait CellRunner: Send + Sync + 'static {
    async fn call_zome(
        &self,
        provenance: &AgentPubKey,
        cap_secret: Option<CapSecret>,
        cell_id: CellId,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: ExternIO,
    ) -> anyhow::Result<ExternIO>;
}

/// The set of all Conductor Services available to the conductor
#[derive(Clone, Default)]
pub struct ConductorServices {
    /// The DPKI service
    pub dpki: Option<Arc<tokio::sync::Mutex<dyn DpkiService>>>,
    /// The AppStore service
    pub app_store: Option<Arc<dyn AppStoreService>>,
}

/// Initialized for ConductorService: just the CellIds that are used for each service
pub struct ConductorServiceCells {
    /// The CellId to use for DPKI
    pub dpki: CellId,
    /// The CellId to use for the AppStore
    pub app_store: CellId,
}
