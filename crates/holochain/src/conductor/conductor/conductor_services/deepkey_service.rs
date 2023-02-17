use std::sync::Arc;

use holo_hash::AgentPubKey;
use holochain_zome_types::{CellId, Timestamp};

use crate::conductor::ConductorHandle;

/// Interface for the Deepkey service
#[async_trait::async_trait]
#[mockall::automock]
pub trait DeepkeyService: Send + Sync {
    /// Check if the key is valid (properly created and not revoked) as-at the given Timestamp
    async fn is_key_valid(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DeepkeyServiceResult<bool>;

    /// Defines the different ways that keys can be created and destroyed:
    /// If an old_key is specified, it will be destroyed
    /// If a new key is specified, it will be registered
    /// If both a new and an old key are specified, the new key will atomically replace the old key
    /// (If no keys are specified, nothing will happen)
    async fn key_mutation(
        &self,
        old_key: Option<AgentPubKey>,
        new_key: Option<AgentPubKey>,
    ) -> DeepkeyServiceResult<()>;
}

/// The errors which can be produced by Deepkey
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum DeepkeyServiceError {
    #[error("Deepkey DNA could not be called: {0}")]
    ZomeCallFailed(String),
}
/// Alias
pub type DeepkeyServiceResult<T> = Result<T, DeepkeyServiceError>;

/// Some more helpful methods built around the methods provided by the service
#[async_trait::async_trait]
pub trait DeepkeyServiceExt: DeepkeyService {
    /// Register a newly created key with Deepkey
    async fn register_key(&self, key: AgentPubKey) -> DeepkeyServiceResult<()> {
        self.key_mutation(None, Some(key)).await
    }

    /// Replace an old key with a new one
    async fn update_key(
        &self,
        old_key: AgentPubKey,
        new_key: AgentPubKey,
    ) -> DeepkeyServiceResult<()> {
        self.key_mutation(Some(old_key), Some(new_key)).await
    }

    /// Replace an old key with a new one
    async fn remove_key(&self, key: AgentPubKey) -> DeepkeyServiceResult<()> {
        self.key_mutation(Some(key), None).await
    }
}

/// The built-in implementation of the deepkey service, which runs a DNA
pub struct DeepkeyBuiltin {
    conductor: ConductorHandle,
    cell_id: CellId,
}

impl DeepkeyBuiltin {
    /// Constructor
    pub fn new(conductor: ConductorHandle, cell_id: CellId) -> Arc<Self> {
        Arc::new(Self { conductor, cell_id })
    }
}

#[async_trait::async_trait]
impl DeepkeyService for DeepkeyBuiltin {
    async fn is_key_valid(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DeepkeyServiceResult<bool> {
        let keystore = self.conductor.keystore();
        let cell_id = self.cell_id.clone();
        let zome_name: String = todo!("depends on dna implementation");
        let fn_name: String = todo!("depends on dna implementation");
        let payload = todo!("depends on dna implementation");
        let cap_secret = None;
        let provenance = cell_id.agent_pubkey();
        let is_valid: bool = self
            .conductor
            .easy_call_zome(provenance, cap_secret, cell_id, zome_name, fn_name, payload)
            .await
            .map_err(|e| DeepkeyServiceError::ZomeCallFailed(e.to_string()))?;
        Ok(is_valid)
    }

    async fn key_mutation(
        &self,
        old_key: Option<AgentPubKey>,
        new_key: Option<AgentPubKey>,
    ) -> DeepkeyServiceResult<()> {
        todo!()
    }
}
