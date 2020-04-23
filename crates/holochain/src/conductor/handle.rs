use super::{
    api::{error::ConductorApiResult, CellConductorApi},
    dna_store::DnaStore,
    error::ConductorResult,
    Cell, Conductor,
};
use std::sync::Arc;
use sx_types::{
    dna::Dna,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    prelude::*,
};
use tokio::sync::RwLock;
use derive_more::From;


pub type ConductorHandle = Arc<dyn ConductorHandleT>;
pub type ConductorHandleInner<DS> = RwLock<Conductor<DS>>;

#[async_trait::async_trait]
pub trait ConductorHandleT: Send + Sync {
    async fn check_running(&self) -> ConductorResult<()>;
    async fn install_dna(&self, dna: Dna) -> ConductorResult<()>;
    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>>;
    async fn invoke_zome(
        &self,
        api: CellConductorApi,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;
}

/// A handle to the conductor that can easily be passed
/// around and cheaply cloned
#[derive(From)]
pub struct ConductorHandleImpl<DS: DnaStore + 'static>(ConductorHandleInner<DS>);

#[async_trait::async_trait]
impl<DS: DnaStore + 'static> ConductorHandleT for ConductorHandleImpl<DS> {
    /// Check that shutdown has not been called
    async fn check_running(&self) -> ConductorResult<()> {
        self.0.read().await.check_running()
    }

    async fn install_dna(&self, dna: Dna) -> ConductorResult<()> {
        unimplemented!()
    }

    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>> {
        unimplemented!()
    }

    async fn invoke_zome(
        &self,
        api: CellConductorApi,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let conductor = self.0.read().await;
        let cell: &Cell = conductor.cell_by_id(&invocation.cell_id)?;
        cell.invoke_zome(api, invocation).await.map_err(Into::into)
    }
}
