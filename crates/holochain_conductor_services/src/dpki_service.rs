use std::sync::Arc;

use holochain_types::prelude::*;

pub mod derivation_paths;

mod deepkey;
pub use deepkey::*;

/// This magic string, when used as the installed app id, denotes that the app
/// is not actually an app, but the DPKI service! This is now a reserved app id,
/// and is used to distinguish the DPKI service from other apps.
pub const DPKI_APP_ID: &str = "DPKI";

pub type DpkiMutex = Arc<tokio::sync::Mutex<dyn DpkiService>>;

/// Interface for the DPKI service
#[async_trait::async_trait]
#[mockall::automock]
#[allow(clippy::needless_lifetimes)]
pub trait DpkiService: Send + Sync {
    /// Check if the key is valid (properly created and not revoked) as-at the given Timestamp
    async fn key_state(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DpkiServiceResult<KeyState>;

    /// Derive a new key in lair using the given index, and register it with DPKI
    async fn derive_and_register_new_key(
        &self,
        app_name: InstalledAppId,
        dna_hash: DnaHash,
    ) -> DpkiServiceResult<AgentPubKey>;

    /// The CellId which backs this service
    fn cell_id(&self) -> &CellId;
}

/// Mirrors the output type of the "key_state" zome function in dpki
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyState {
    NotFound,
    Invalidated(SignedActionHashed),
    Valid(SignedActionHashed),
}

impl KeyState {
    pub fn is_valid(&self) -> bool {
        matches!(self, KeyState::Valid(_))
    }
}

/// The errors which can be produced by DPKI
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum DpkiServiceError {
    #[error("DPKI DNA could not be called: {0}")]
    ZomeCallFailed(anyhow::Error),
    #[error(transparent)]
    Serialization(#[from] SerializedBytesError),
    #[error("Error talking to lair keystore: {0}")]
    Lair(anyhow::Error),
}
/// Alias
pub type DpkiServiceResult<T> = Result<T, DpkiServiceError>;

/// Create a minimal usable mock of DPKI
#[cfg(feature = "fuzzing")]
pub fn mock_dpki() -> MockDpkiService {
    use arbitrary::Arbitrary;
    use futures::FutureExt;

    let mut dpki = MockDpkiService::new();
    let mut u = unstructured_noise();
    let action = SignedActionHashed::arbitrary(&mut u).unwrap();
    dpki.expect_key_state().returning(move |_, _| {
        let action = action.clone();
        async move { Ok(KeyState::Valid(action)) }.boxed()
    });
    dpki.expect_cell_id().return_const(fake_cell_id(0));
    dpki
}
