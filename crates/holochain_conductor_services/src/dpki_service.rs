use std::sync::Arc;

use holochain_types::prelude::*;

pub mod derivation_paths;

pub(crate) mod zome_types;

pub use zome_types::*;

mod deepkey;
pub use deepkey::*;

use crate::CellRunner;

/// This magic string, when used as the installed app id, denotes that the app
/// is not actually an app, but the DPKI service! This is now a reserved app id,
/// and is used to distinguish the DPKI service from other apps.
pub const DPKI_APP_ID: &str = "DPKI";

pub type DpkiImpl = Arc<DpkiService>;

pub struct DpkiService {
    /// Mirrored from the State
    pub uuid: [u8; 32],

    /// Mirrored from the State
    pub cell_id: Option<CellId>,

    pub device_seed_lair_tag: String,

    /// State must be accessed through a Mutex
    pub state: tokio::sync::Mutex<Box<dyn DpkiState>>,
}

// /// Interface for the DPKI service
impl DpkiService {
    pub fn should_run(&self, dna_hash: &DnaHash) -> bool {
        if let Some(cell_id) = self.cell_id.as_ref() {
            cell_id.dna_hash() != dna_hash
        } else {
            true
        }
    }

    pub fn new_deepkey(installation: DeepkeyInstallation, runner: Arc<impl CellRunner>) -> Self {
        let state: Box<dyn DpkiState> = Box::new(DeepkeyState {
            runner,
            cell_id: installation.cell_id.clone(),
        });
        let uuid = installation
            .cell_id
            .dna_hash()
            .get_raw_32()
            .try_into()
            .unwrap();
        let cell_id = Some(installation.cell_id);
        let device_seed_lair_tag = installation.device_seed_lair_tag;
        let state = tokio::sync::Mutex::new(state);
        Self {
            uuid,
            cell_id,
            device_seed_lair_tag,
            state,
        }
    }
}

#[async_trait::async_trait]
#[mockall::automock]
pub trait DpkiState: Send + Sync {
    // fn uuid(&self) -> [u8; 32];

    // /// If the service is backed by a cell, return the CellId
    // fn cell_id(&self) -> Option<CellId>;

    async fn next_derivation_details(
        &self,
        app_name: InstalledAppId,
    ) -> DpkiServiceResult<DerivationDetailsInput>;

    async fn register_key(
        &self,
        input: CreateKeyInput,
    ) -> DpkiServiceResult<(ActionHash, KeyRegistration, KeyMeta)>;

    /// Check if the key is valid (properly created and not revoked) as-at the given Timestamp
    async fn key_state(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DpkiServiceResult<KeyState>;
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
