use std::sync::Arc;

use holochain_types::prelude::*;

pub mod derivation_paths;

pub(crate) mod zome_types;

pub use zome_types::KeyState;

mod deepkey;
pub use deepkey::*;

use self::zome_types::*;

/// This magic string, when used as the installed app id, denotes that the app
/// is not actually an app, but the DPKI service! This is now a reserved app id,
/// and is used to distinguish the DPKI service from other apps.
pub const DPKI_APP_ID: &str = "DPKI";

pub type DpkiImpl = Arc<dyn DpkiService>;

/// Interface for the DPKI service
#[async_trait::async_trait]
#[mockall::automock]
#[allow(clippy::needless_lifetimes)]
pub trait DpkiService: Send + Sync {
    fn uuid(&self) -> [u8; 32];

    fn dpki_agent(&self) -> AgentPubKey;

    /// Allows the DPKI service to determine if it should run for a given DNA.
    ///
    /// This is primarily to allow a DNA-backed DPKI service to not run on itself
    /// while it is being installed, which leads to deadlock.
    fn should_run(&self, dna_hash: &DnaHash) -> bool {
        dna_hash.get_raw_32() != self.uuid()
    }

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
        dna_hashes: Vec<DnaHash>,
    ) -> DpkiServiceResult<AgentPubKey>;
}

#[async_trait::async_trait]
#[mockall::automock]
pub trait DpkiDerivation: Send + Sync {
    async fn next_derivation_details(
        &self,
        app_name: InstalledAppId,
    ) -> DpkiServiceResult<DerivationDetailsInput>;

    async fn create_key(
        &self,
        input: CreateKeyInput,
    ) -> DpkiServiceResult<(ActionHash, KeyRegistration, KeyMeta)>;
}

#[async_trait::async_trait]
pub trait DpkiServiceExt: DpkiService {}
impl<T> DpkiServiceExt for T where T: DpkiService {}

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
