//! The CellConductorApi allows Cells to talk to their Conductor

use super::error::ConductorApiError;
use super::error::ConductorApiResult;
use crate::conductor::error::ConductorResult;
use crate::conductor::ConductorHandle;
use crate::core::ribosome::guest_callback::post_commit::PostCommitArgs;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::workflow::ZomeCallResult;
use async_trait::async_trait;
use holochain_keystore::MetaLairClient;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_state::nonce::WitnessNonceResult;
use holochain_state::prelude::DatabaseResult;
use holochain_types::prelude::*;
use holochain_zome_types::block::Block;
use holochain_zome_types::block::BlockTargetId;
use std::sync::Arc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::OwnedPermit;

/// The concrete implementation of [`CellConductorApiT`], which is used to give
/// Cells an API for calling back to their [`Conductor`](crate::conductor::Conductor).
#[derive(Clone)]
pub struct CellConductorApi {
    conductor_handle: ConductorHandle,
    cell_id: CellId,
}

/// Alias
pub type CellConductorHandle = Arc<dyn CellConductorApiT + Send + 'static>;

/// A minimal set of functionality needed from the conductor by
/// host functions.
pub type CellConductorReadHandle = Arc<dyn CellConductorReadHandleT + Send + 'static>;

impl CellConductorApi {
    /// Instantiate from a Conductor reference and a CellId to identify which Cell
    /// this API instance is associated with
    pub fn new(conductor_handle: ConductorHandle, cell_id: CellId) -> Self {
        Self {
            conductor_handle,
            cell_id,
        }
    }
}

#[async_trait]
impl CellConductorApiT for CellConductorApi {
    fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    fn keystore(&self) -> &MetaLairClient {
        self.conductor_handle.keystore()
    }

    fn get_dna_file(&self, cell_id: &CellId) -> Option<DnaFile> {
        self.conductor_handle.get_dna_file(cell_id)
    }

    fn get_this_ribosome(&self) -> ConductorApiResult<RealRibosome> {
        Ok(self.conductor_handle.get_ribosome(&self.cell_id)?)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    fn get_zome(&self, cell_id: &CellId, zome_name: &ZomeName) -> ConductorApiResult<Zome> {
        let dna = self
            .get_dna_file(cell_id)
            .ok_or_else(|| ConductorApiError::CellMissing(cell_id.clone()))?;
        Ok(dna.dna_def().get_zome(zome_name)?)
    }

    fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef> {
        self.conductor_handle.get_entry_def(key)
    }

    fn into_call_zome_handle(self) -> CellConductorReadHandle {
        Arc::new(self)
    }

    async fn post_commit_permit(&self) -> Result<OwnedPermit<PostCommitArgs>, SendError<()>> {
        self.conductor_handle.post_commit_permit().await
    }
}

/// The "internal" Conductor API interface, for a Cell to talk to its calling Conductor.
#[async_trait]
#[cfg_attr(feature = "test_utils", mockall::automock)]
pub trait CellConductorApiT: Send + Sync {
    /// Get this cell id
    fn cell_id(&self) -> &CellId;

    /// Request access to this conductor's keystore
    fn keystore(&self) -> &MetaLairClient;

    /// Get a [`Dna`](holochain_types::prelude::Dna) from the [`RibosomeStore`](crate::conductor::ribosome_store::RibosomeStore)
    fn get_dna_file(&self, cell_id: &CellId) -> Option<DnaFile>;

    /// Get the [`RealRibosome`] of this cell from the [`RibosomeStore`](crate::conductor::ribosome_store::RibosomeStore)
    fn get_this_ribosome(&self) -> ConductorApiResult<RealRibosome>;

    /// Get a [`Zome`](holochain_types::prelude::Zome) from this cell's Dna
    fn get_zome(&self, cell_id: &CellId, zome_name: &ZomeName) -> ConductorApiResult<Zome>;

    /// Get a [`EntryDef`] from the [`EntryDefBufferKey`]
    fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef>;

    /// Turn this into a call zome handle
    fn into_call_zome_handle(self) -> CellConductorReadHandle;

    /// Get an OwnedPermit to the post commit task.
    async fn post_commit_permit(&self) -> Result<OwnedPermit<PostCommitArgs>, SendError<()>>;
}

/// A minimal set of functionality needed from the conductor by
/// host functions.
#[async_trait]
#[cfg_attr(feature = "test_utils", mockall::automock)]
pub trait CellConductorReadHandleT: Send + Sync {
    /// Get this cell id
    fn cell_id(&self) -> &CellId;

    /// Invoke a zome function on a Cell
    async fn call_zome(&self, params: ZomeCallParams) -> ConductorApiResult<ZomeCallResult>;

    /// Invoke a zome function on a Cell
    async fn call_zome_with_workspace(
        &self,
        params: ZomeCallParams,
        workspace_lock: SourceChainWorkspace,
    ) -> ConductorApiResult<ZomeCallResult>;

    /// Get a zome from this cell's Dna
    fn get_zome(&self, cell_id: &CellId, zome_name: &ZomeName) -> ConductorApiResult<Zome>;

    /// Get a [`EntryDef`] from the [`EntryDefBufferKey`]
    fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef>;

    /// Try to put the nonce from a calling agent in the db. Fails with a stale result if a newer nonce exists.
    async fn witness_nonce_from_calling_agent(
        &self,
        agent: AgentPubKey,
        nonce: Nonce256Bits,
        expires: Timestamp,
    ) -> ConductorApiResult<WitnessNonceResult>;

    /// Find the first cell ID across all apps the given cell id is in that
    /// is assigned to the given role.
    async fn find_cell_with_role_alongside_cell(
        &self,
        cell_id: &CellId,
        role_name: &RoleName,
    ) -> ConductorResult<Option<CellId>>;

    /// Expose block functionality to zomes.
    async fn block(&self, input: Block) -> DatabaseResult<()>;

    /// Expose unblock functionality to zomes.
    async fn unblock(&self, input: Block) -> DatabaseResult<()>;

    /// Expose is_blocked functionality to zomes.
    async fn is_blocked(&self, input: BlockTargetId, timestamp: Timestamp)
        -> ConductorResult<bool>;

    /// Find an installed app by one of its [CellId]s.
    async fn find_app_containing_cell(
        &self,
        cell_id: &CellId,
    ) -> ConductorResult<Option<InstalledApp>>;

    /// Expose create_clone_cell functionality to zomes.
    async fn create_clone_cell(
        &self,
        installed_app_id: &InstalledAppId,
        payload: CreateCloneCellPayload,
    ) -> ConductorResult<ClonedCell>;

    /// Expose disable_clone_cell functionality to zomes.
    async fn disable_clone_cell(
        &self,
        installed_app_id: &InstalledAppId,
        payload: DisableCloneCellPayload,
    ) -> ConductorResult<()>;

    /// Expose enable_clone_cell functionality to zomes.
    async fn enable_clone_cell(
        &self,
        installed_app_id: &InstalledAppId,
        payload: EnableCloneCellPayload,
    ) -> ConductorResult<ClonedCell>;

    /// Expose delete_clone_cell functionality to zomes.
    async fn delete_clone_cell(&self, payload: DeleteCloneCellPayload) -> ConductorResult<()>;

    /// Accept a countersigning session.
    #[cfg(feature = "unstable-countersigning")]
    async fn accept_countersigning_session(
        &self,
        cell_id: CellId,
        request: PreflightRequest,
    ) -> ConductorResult<PreflightRequestAcceptance>;
}

#[async_trait]
impl CellConductorReadHandleT for CellConductorApi {
    fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    async fn call_zome(&self, params: ZomeCallParams) -> ConductorApiResult<ZomeCallResult> {
        self.conductor_handle.call_zome(params).await
    }

    async fn call_zome_with_workspace(
        &self,
        params: ZomeCallParams,
        workspace_lock: SourceChainWorkspace,
    ) -> ConductorApiResult<ZomeCallResult> {
        if self.cell_id == params.cell_id {
            self.conductor_handle
                .call_zome_with_workspace(params, workspace_lock)
                .await
        } else {
            self.conductor_handle.call_zome(params).await
        }
    }

    fn get_zome(&self, cell_id: &CellId, zome_name: &ZomeName) -> ConductorApiResult<Zome> {
        CellConductorApiT::get_zome(self, cell_id, zome_name)
    }

    fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef> {
        CellConductorApiT::get_entry_def(self, key)
    }

    async fn witness_nonce_from_calling_agent(
        &self,
        agent: AgentPubKey,
        nonce: Nonce256Bits,
        expires: Timestamp,
    ) -> ConductorApiResult<WitnessNonceResult> {
        Ok(self
            .conductor_handle
            .witness_nonce_from_calling_agent(agent, nonce, expires)
            .await?)
    }

    async fn find_cell_with_role_alongside_cell(
        &self,
        cell_id: &CellId,
        role_name: &RoleName,
    ) -> ConductorResult<Option<CellId>> {
        self.conductor_handle
            .find_cell_with_role_alongside_cell(cell_id, role_name)
            .await
    }

    async fn block(&self, input: Block) -> DatabaseResult<()> {
        self.conductor_handle.block(input).await
    }

    async fn unblock(&self, input: Block) -> DatabaseResult<()> {
        self.conductor_handle.unblock(input).await
    }

    async fn is_blocked(
        &self,
        input: BlockTargetId,
        timestamp: Timestamp,
    ) -> ConductorResult<bool> {
        self.conductor_handle.is_blocked(input, timestamp).await
    }

    async fn find_app_containing_cell(
        &self,
        cell_id: &CellId,
    ) -> ConductorResult<Option<InstalledApp>> {
        self.conductor_handle
            .find_app_containing_cell(cell_id)
            .await
    }

    async fn create_clone_cell(
        &self,
        installed_app_id: &InstalledAppId,
        payload: CreateCloneCellPayload,
    ) -> ConductorResult<ClonedCell> {
        self.conductor_handle
            .clone()
            .create_clone_cell(installed_app_id, payload)
            .await
    }

    async fn disable_clone_cell(
        &self,
        installed_app_id: &InstalledAppId,
        payload: DisableCloneCellPayload,
    ) -> ConductorResult<()> {
        self.conductor_handle
            .clone()
            .disable_clone_cell(installed_app_id, &payload)
            .await
    }

    async fn enable_clone_cell(
        &self,
        installed_app_id: &InstalledAppId,
        payload: EnableCloneCellPayload,
    ) -> ConductorResult<ClonedCell> {
        self.conductor_handle
            .clone()
            .enable_clone_cell(installed_app_id, &payload)
            .await
    }

    async fn delete_clone_cell(&self, payload: DeleteCloneCellPayload) -> ConductorResult<()> {
        self.conductor_handle
            .clone()
            .delete_clone_cell(&payload)
            .await
    }

    #[cfg(feature = "unstable-countersigning")]
    async fn accept_countersigning_session(
        &self,
        cell_id: CellId,
        request: PreflightRequest,
    ) -> ConductorResult<PreflightRequestAcceptance> {
        self.conductor_handle
            .accept_countersigning_session(cell_id, request)
            .await
    }
}
