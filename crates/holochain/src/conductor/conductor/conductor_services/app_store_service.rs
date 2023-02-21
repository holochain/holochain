use std::sync::Arc;

use holochain_types::prelude::*;

use crate::conductor::ConductorHandle;

/// Interface for the AppStore service
#[async_trait::async_trait]
#[mockall::automock]
pub trait AppStoreService: Send + Sync {
    /// Fetch a DNA bundle from the store
    async fn get_dna_bundle(&self, dna_hash: DnaHash) -> AppStoreServiceResult<Option<DnaBundle>>;

    /// Fetch an app bundle from the store
    async fn get_app_bundle(&self, app_hash: AppHash) -> AppStoreServiceResult<Option<AppBundle>>;

    /// The CellIds in use by this service, which need to be protected
    fn cell_ids<'a>(&'a self) -> std::collections::HashSet<&'a CellId>;
}

/// The errors which can be produced by the AppStoreService
#[derive(thiserror::Error, Debug)]
pub enum AppStoreServiceError {}
/// Alias
pub type AppStoreServiceResult<T> = Result<T, AppStoreServiceError>;

/// This doesn't exist yet. We need to define it.
pub enum AppHash {}

/// The built-in implementation of the app store service, which runs a DNA
pub struct AppStoreBuiltin {
    _conductor: ConductorHandle,
    _cell_id: CellId,
}

impl AppStoreBuiltin {
    /// Constructor
    pub fn new(conductor: ConductorHandle, cell_id: CellId) -> Arc<Self> {
        Arc::new(Self {
            _conductor: conductor,
            _cell_id: cell_id,
        })
    }
}

#[async_trait::async_trait]
impl AppStoreService for AppStoreBuiltin {
    async fn get_dna_bundle(&self, _dna_hash: DnaHash) -> AppStoreServiceResult<Option<DnaBundle>> {
        todo!("placeholder")
    }

    async fn get_app_bundle(&self, _app_hash: AppHash) -> AppStoreServiceResult<Option<AppBundle>> {
        todo!("placeholder")
    }

    fn cell_ids<'a>(&'a self) -> std::collections::HashSet<&'a CellId> {
        [&self._cell_id].into_iter().collect()
    }
}

/// Create a minimal usable mock of the app store
pub fn mock_app_store() -> MockAppStoreService {
    let mut app_store = MockAppStoreService::new();
    app_store
        .expect_cell_ids()
        .return_const(std::collections::HashSet::new());
    app_store
}
