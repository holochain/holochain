//! The CellConductorApi allows Cells to talk to their Conductor

use super::error::{ConductorApiError, ConductorApiResult};
use crate::conductor::ConductorHandle;
use async_trait::async_trait;
use holo_hash::DnaHash;
use holochain_keystore::KeystoreSender;
use holochain_types::{
    autonomic::AutonomicCue,
    cell::CellId,
    dna::Dna,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    prelude::Todo,
};

/// The concrete implementation of [CellConductorApiT], which is used to give
/// Cells an API for calling back to their [Conductor].
#[derive(Clone)]
pub struct CellConductorApi {
    conductor_handle: ConductorHandle,
    cell_id: CellId,
}

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
    async fn invoke_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        if *cell_id == invocation.cell_id {
            self.conductor_handle
                .invoke_zome(invocation)
                .await
                .map_err(Into::into)
        } else {
            Err(ConductorApiError::ZomeInvocationCellMismatch {
                api_cell_id: cell_id.clone(),
                invocation_cell_id: invocation.cell_id,
            })
        }
    }

    async fn dpki_request(&self, _method: String, _args: String) -> ConductorApiResult<String> {
        unimplemented!()
    }

    async fn network_send(&self, _message: Todo) -> ConductorApiResult<()> {
        unimplemented!()
    }

    async fn network_request(&self, _message: Todo) -> ConductorApiResult<Todo> {
        unimplemented!()
    }

    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()> {
        self.conductor_handle
            .autonomic_cue(cue, &self.cell_id)
            .await
    }

    fn keystore(&self) -> &KeystoreSender {
        self.conductor_handle.keystore()
    }

    async fn get_dna(&self, dna_hash: DnaHash) -> Option<Dna> {
        self.conductor_handle.get_dna(dna_hash).await
    }
}

/// The "internal" Conductor API interface, for a Cell to talk to its calling Conductor.
#[async_trait]
pub trait CellConductorApiT: Clone + Send + Sync + Sized {
    /// Invoke a zome function on any cell in this conductor.
    /// An invocation on a different Cell than this one corresponds to a bridged call.
    async fn invoke_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

    /// Make a request to the DPKI service running for this Conductor.
    /// TODO: decide on actual signature
    async fn dpki_request(&self, method: String, args: String) -> ConductorApiResult<String>;

    /// Send a message to the network engine, ignoring the response
    async fn network_send(&self, message: Todo) -> ConductorApiResult<()>;

    /// Send a message to the network engine, and await the response
    async fn network_request(&self, _message: Todo) -> ConductorApiResult<Todo>;

    /// Cue the autonomic system to run an [AutonomicProcess] earlier than its scheduled time.
    /// This is basically a heuristic designed to help things run more smoothly.
    async fn autonomic_cue(&self, cue: AutonomicCue) -> ConductorApiResult<()>;

    /// Request access to this conductor's keystore
    fn keystore(&self) -> &KeystoreSender;

    /// Get a [Dna] from the [DnaStore]
    async fn get_dna(&self, dna_hash: DnaHash) -> Option<Dna>;
}
