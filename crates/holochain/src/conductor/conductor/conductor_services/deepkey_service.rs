use holo_hash::AgentPubKey;
use holochain_zome_types::Timestamp;

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
pub enum DeepkeyServiceError {}
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
