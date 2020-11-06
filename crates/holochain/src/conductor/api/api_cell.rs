//! The CellConductorApi allows Cells to talk to their Conductor

use std::sync::Arc;

use super::error::{ConductorApiError, ConductorApiResult};
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::ZomeCallInvocationResult;
use crate::{
    conductor::{
        entry_def_store::EntryDefBufferKey, interface::SignalBroadcaster, ConductorHandle,
    },
    core::workflow::CallZomeWorkspaceLock,
};
use async_trait::async_trait;
use holo_hash::DnaHash;
use holochain_keystore::KeystoreSender;
use holochain_types::{autonomic::AutonomicCue, cell::CellId, dna::DnaFile};
use holochain_zome_types::entry_def::EntryDef;
use tracing::*;

/// The concrete implementation of [CellConductorApiT], which is used to give
/// Cells an API for calling back to their [Conductor].
#[derive(Clone)]
pub struct CellConductorApi {
    conductor_handle: ConductorHandle,
    cell_id: CellId,
}

/// A handle that cn only call zome functions to avoid
/// making write lock calls
pub type CellConductorReadHandle = Arc<dyn CellConductorReadHandleT>;

impl CellConductorApi {
    /// Instantiate from a Conductor reference and a CellId to identify which Cell
    /// this API instance is associated with
    pub fn new(conductor_handle: ConductorHandle, cell_id: CellId) -> Self {
        Self {
            cell_id,
            conductor_handle,
        }
    }
}

#[async_trait]
impl CellConductorApiT for CellConductorApi {
    fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    async fn call_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeCallInvocation,
    ) -> ConductorApiResult<ZomeCallInvocationResult> {
        if *cell_id == invocation.cell_id {
            self.conductor_handle
                .call_zome(invocation)
                .await
                .map_err(Into::into)
        } else {
            Err(ConductorApiError::ZomeCallInvocationCellMismatch {
                api_cell_id: cell_id.clone(),
                invocation_cell_id: invocation.cell_id,
            })
        }
    }

    async fn dpki_request(&self, _method: String, _args: String) -> ConductorApiResult<String> {
        warn!("Using placeholder dpki");
        Ok("TODO".to_string())
    }

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()> {
        self.conductor_handle
            .autonomic_cue(cue, &self.cell_id)
            .await
    }

    fn keystore(&self) -> &KeystoreSender {
        self.conductor_handle.keystore()
    }

    async fn signal_broadcaster(&self) -> SignalBroadcaster {
        self.conductor_handle.signal_broadcaster().await
    }

    async fn get_dna(&self, dna_hash: &DnaHash) -> Option<DnaFile> {
        self.conductor_handle.get_dna(dna_hash).await
    }

    async fn get_this_dna(&self) -> Option<DnaFile> {
        self.conductor_handle.get_dna(self.cell_id.dna_hash()).await
    }

    async fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef> {
        self.conductor_handle.get_entry_def(key).await
    }

    fn into_call_zome_handle(self) -> CellConductorReadHandle {
        Arc::new(self)
    }
}

/// The "internal" Conductor API interface, for a Cell to talk to its calling Conductor.
#[async_trait]
pub trait CellConductorApiT: Clone + Send + Sync + Sized {
    /// Get this cell id
    fn cell_id(&self) -> &CellId;

    /// Invoke a zome function on any cell in this conductor.
    /// An invocation on a different Cell than this one corresponds to a bridged call.
    async fn call_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeCallInvocation,
    ) -> ConductorApiResult<ZomeCallInvocationResult>;

    /// Make a request to the DPKI service running for this Conductor.
    /// TODO: decide on actual signature
    async fn dpki_request(&self, method: String, args: String) -> ConductorApiResult<String>;

    /// Cue the autonomic system to run an [AutonomicProcess] earlier than its scheduled time.
    /// This is basically a heuristic designed to help things run more smoothly.
    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()>;

    /// Request access to this conductor's keystore
    fn keystore(&self) -> &KeystoreSender;

    /// Access the broadcast Sender which will send a Signal across every
    /// attached app interface
    async fn signal_broadcaster(&self) -> SignalBroadcaster;

    /// Get a [Dna] from the [DnaStore]
    async fn get_dna(&self, dna_hash: &DnaHash) -> Option<DnaFile>;

    /// Get the [Dna] of this cell from the [DnaStore]
    async fn get_this_dna(&self) -> Option<DnaFile>;

    /// Get a [EntryDef] from the [EntryDefBuf]
    async fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef>;

    /// Turn this into a call zome handle
    fn into_call_zome_handle(self) -> CellConductorReadHandle;
}

#[async_trait]
/// A handle that cn only call zome functions to avoid
/// making write lock calls
pub trait CellConductorReadHandleT: Send + Sync {
    /// Get this cell id
    fn cell_id(&self) -> &CellId;
    /// Invoke a zome function on a Cell
    async fn call_zome(
        &self,
        invocation: ZomeCallInvocation,
        workspace_lock: &CallZomeWorkspaceLock,
    ) -> ConductorApiResult<ZomeCallInvocationResult>;
}

#[async_trait]
impl CellConductorReadHandleT for CellConductorApi {
    fn cell_id(&self) -> &CellId {
        &self.cell_id
    }
    async fn call_zome(
        &self,
        invocation: ZomeCallInvocation,
        workspace_lock: &CallZomeWorkspaceLock,
    ) -> ConductorApiResult<ZomeCallInvocationResult> {
        if self.cell_id == invocation.cell_id {
            self.conductor_handle
                .call_zome_with_workspace(invocation, workspace_lock.clone())
                .await
        } else {
            self.conductor_handle.call_zome(invocation).await
        }
    }
}
