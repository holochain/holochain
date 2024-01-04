use std::sync::Arc;

use holochain_keystore::MetaLairClient;
use holochain_types::prelude::*;

use crate::CellRunner;

/// Interface for the DPKI service
#[async_trait::async_trait]
#[mockall::automock]
#[allow(clippy::needless_lifetimes)]
pub trait DpkiService: Send + Sync {
    /// Check if the key is valid (properly created and not revoked) as-at the given Timestamp
    async fn is_key_valid(&self, key: AgentPubKey, timestamp: Timestamp)
        -> DpkiServiceResult<bool>;

    /// Defines the different ways that keys can be created and destroyed:
    /// If an old key is specified, it will be destroyed
    /// If a new key is specified, it will be registered
    /// If both a new and an old key are specified, the new key will atomically replace the old key
    /// (If no keys are specified, nothing will happen)
    async fn key_mutation(
        &self,
        old_key: Option<AgentPubKey>,
        new_key: Option<AgentPubKey>,
    ) -> DpkiServiceResult<()>;

    /// The CellIds in use by this service, which need to be protected
    fn cell_ids<'a>(&'a self) -> std::collections::HashSet<&'a CellId>;
}

/// The errors which can be produced by DPKI
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum DpkiServiceError {
    #[error("DPKI DNA could not be called: {0}")]
    ZomeCallFailed(anyhow::Error),
}
/// Alias
pub type DpkiServiceResult<T> = Result<T, DpkiServiceError>;

/// Some more helpful methods built around the methods provided by the service
#[async_trait::async_trait]
pub trait DpkiServiceExt: DpkiService {
    /// Register a newly created key with DPKI
    async fn register_key(&self, key: AgentPubKey) -> DpkiServiceResult<()> {
        self.key_mutation(None, Some(key)).await
    }

    /// Replace an old key with a new one
    async fn update_key(
        &self,
        old_key: AgentPubKey,
        new_key: AgentPubKey,
    ) -> DpkiServiceResult<()> {
        self.key_mutation(Some(old_key), Some(new_key)).await
    }

    /// Delete an existing key without replacing it with a new one.
    /// This effectively terminates the "lineage" that this key was a part of.
    async fn remove_key(&self, key: AgentPubKey) -> DpkiServiceResult<()> {
        self.key_mutation(Some(key), None).await
    }
}

/// The built-in implementation of the DPKI service contract, which runs a DNA
#[derive(derive_more::Constructor)]
pub struct DeepkeyBuiltin {
    runner: Arc<dyn CellRunner>,
    keystore: MetaLairClient,
    cell_id: CellId,
}

#[allow(unreachable_code)]
#[allow(unused_variables)]
#[allow(clippy::needless_lifetimes)]
#[async_trait::async_trait]
impl DpkiService for DeepkeyBuiltin {
    async fn is_key_valid(
        &self,
        key: AgentPubKey,
        timestamp: Timestamp,
    ) -> DpkiServiceResult<bool> {
        let keystore = self.keystore.clone();
        let cell_id = self.cell_id.clone();
        let zome_name: ZomeName = "TODO: depends on dna implementation".into();
        let fn_name: FunctionName = "TODO: depends on dna implementation".into();
        let payload = todo!("TODO: depends on dna implementation");
        let cap_secret = None;
        let provenance = cell_id.agent_pubkey().clone();
        let response = self
            .runner
            .call_zome(
                &provenance,
                cap_secret,
                cell_id,
                zome_name,
                fn_name,
                payload,
            )
            .await
            .map_err(DpkiServiceError::ZomeCallFailed)?;
        let is_valid = todo!("deserialize response");
        Ok(is_valid)
    }

    async fn key_mutation(
        &self,
        old_key: Option<AgentPubKey>,
        new_key: Option<AgentPubKey>,
    ) -> DpkiServiceResult<()> {
        todo!()
    }

    fn cell_ids<'a>(&'a self) -> std::collections::HashSet<&'a CellId> {
        [&self.cell_id].into_iter().collect()
    }
}

/// Create a minimal usable mock of DPKI
pub fn mock_dpki() -> MockDpkiService {
    use futures::FutureExt;
    let mut dpki = MockDpkiService::new();
    dpki.expect_is_key_valid()
        .returning(|_, _| async move { Ok(true) }.boxed());
    dpki.expect_cell_ids()
        .return_const(std::collections::HashSet::new());
    dpki
}
