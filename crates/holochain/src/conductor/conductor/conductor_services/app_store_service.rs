use holochain_types::prelude::*;

/// Interface for the AppStore service
#[async_trait::async_trait]
// #[mockall::automock]
pub trait AppStoreService: Send + Sync {
    /// Fetch a DNA bundle from the store
    async fn get_dna_bundle(&self, dna_hash: DnaHash) -> AppStoreServiceResult<Option<DnaBundle>>;

    /// Fetch an app bundle from the store
    async fn get_app_bundle(&self, app_hash: AppHash) -> AppStoreServiceResult<Option<AppBundle>>;
}

/// The errors which can be produced by the AppStoreService
#[derive(thiserror::Error, Debug)]
pub enum AppStoreServiceError {}
/// Alias
pub type AppStoreServiceResult<T> = Result<T, AppStoreServiceError>;

/// This doesn't exist yet. We need to define it.
pub enum AppHash {}
