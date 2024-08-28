use std::sync::Arc;

pub use hc_deepkey_sdk::*;
use holochain_types::prelude::*;
use holochain_util::timed;

pub mod derivation_paths;

mod deepkey;
pub use deepkey::*;

use crate::CellRunner;

/// This magic string, when used as the installed app id, denotes that the app
/// is not actually an app, but the DPKI service! This is now a reserved app id,
/// and is used to distinguish the DPKI service from other apps.
pub const DPKI_APP_ID: &str = "DPKI";

pub type DpkiImpl = Arc<DpkiService>;

pub struct DpkiService {
    /// Mirrored from the State.
    /// Note, this is a little weird for DPKI implementations which are not backed by a Holochain DNA.
    /// In that case, the impl still needs an AgentPubKey to sign new key registrations with, and it still
    /// needs a unique identifier to advertise network compatibility, which is covered by the DnaHash.
    /// So such an implementation should just use 32 unique bytes and create a DnaHash from that, to be
    /// used in this CellId.
    pub cell_id: CellId,

    pub device_seed_lair_tag: String,

    /// State must be accessed through a Mutex
    state: tokio::sync::Mutex<Box<dyn DpkiState>>,
}

/// Interface for the DPKI service
impl DpkiService {
    pub fn new(
        cell_id: CellId,
        device_seed_lair_tag: String,
        state: impl DpkiState + 'static,
    ) -> Self {
        let state: Box<dyn DpkiState> = Box::new(state);
        let state = tokio::sync::Mutex::new(state);
        Self {
            cell_id,
            device_seed_lair_tag,
            state,
        }
    }

    /// Whether the passed in DNA hash is Deepkey DNA hash
    pub fn is_deepkey_dna(&self, dna_hash: &DnaHash) -> bool {
        self.cell_id.dna_hash() == dna_hash
    }

    /// Get the UUID of the DPKI service.
    pub fn uuid(&self) -> [u8; 32] {
        self.cell_id.dna_hash().get_raw_32().try_into().unwrap()
    }

    pub fn new_deepkey(installation: DeepkeyInstallation, runner: Arc<impl CellRunner>) -> Self {
        let state: Box<dyn DpkiState> = Box::new(DeepkeyState {
            runner,
            cell_id: installation.cell_id.clone(),
        });
        let cell_id = installation.cell_id;
        let device_seed_lair_tag = installation.device_seed_lair_tag;
        let state = tokio::sync::Mutex::new(state);
        Self {
            cell_id,
            device_seed_lair_tag,
            state,
        }
    }

    pub async fn state(&self) -> tokio::sync::MutexGuard<Box<dyn DpkiState>> {
        timed!([1, 10, 1000], { self.state.lock().await })
    }
}

#[async_trait::async_trait]
#[mockall::automock]
pub trait DpkiState: Send + Sync {
    /// If agent key is none, we're registering a new key.
    /// If some, we're about to update an existing key.
    async fn next_derivation_details(
        &self,
        agent_key: Option<AgentPubKey>,
    ) -> DpkiServiceResult<DerivationDetails>;

    /// Create a new key for a given app.
    async fn register_key(
        &self,
        input: CreateKeyInput,
    ) -> DpkiServiceResult<(ActionHash, KeyRegistration, KeyMeta)>;

    /// Query meta data for a given key.
    async fn query_key_meta(&self, key: AgentPubKey) -> DpkiServiceResult<KeyMeta>;

    /// Revoke a registered key.
    async fn revoke_key(
        &self,
        input: RevokeKeyInput,
    ) -> DpkiServiceResult<(ActionHash, KeyRegistration)>;

    /// Check if the key is valid (properly created and not revoked) as-at the given Timestamp.
    async fn key_state(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DpkiServiceResult<KeyState>;

    /// Get lineage of all keys of an agent, ordered by timestamp.
    async fn get_agent_key_lineage(&self, key: AgentPubKey) -> DpkiServiceResult<Vec<AgentPubKey>>;

    /// Check if the two provided agent keys belong to the same agent.
    async fn is_same_agent(
        &self,
        key_1: AgentPubKey,
        key_2: AgentPubKey,
    ) -> DpkiServiceResult<bool>;
}

/// The errors which can be produced by DPKI
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum DpkiServiceError {
    #[error("DPKI DNA call failed: {0}")]
    ZomeCallFailed(anyhow::Error),
    #[error(transparent)]
    Serialization(#[from] SerializedBytesError),
    #[error("Error talking to lair keystore: {0}")]
    Lair(anyhow::Error),
    #[error("DPKI service not installed")]
    DpkiNotInstalled,
    #[error("The agent {0} could not be found in DPKI")]
    DpkiAgentMissing(AgentPubKey),
    #[error("The agent {0} was found to be invalid at {1} according to the DPKI service")]
    DpkiAgentInvalid(AgentPubKey, Timestamp),
}
/// Alias
pub type DpkiServiceResult<T> = Result<T, DpkiServiceError>;
