use super::{dna_store::DnaStore, error::ConductorResult, Conductor, api::{CellConductorApi, error::ConductorApiResult}, Cell};
use std::sync::Arc;
use sx_types::{nucleus::{ZomeInvocationResponse, ZomeInvocation}, prelude::*};
use tokio::sync::RwLock;

#[async_trait::async_trait]
pub trait ConductorHandleT {
    async fn check_running(&self) -> ConductorResult<()>;
    async fn install_dna(&self, dna: Dna) -> ConductorResult<()>;
    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>>;
    async fn invoke_zome(
        &self,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;
}

/// A handle to the conductor that can easily be passed
/// around and cheaply cloned
#[derive(Clone)]
pub struct ConductorHandleImpl<DS: DnaStore>(Arc<RwLock<Box<Conductor<DS>>>>);

#[async_trait::async_trait]
impl<DS: DnaStore> ConductorHandleT for ConductorHandleImpl<DS> {
    /// Check that shutdown has not been called
    async fn check_running(&self) -> ConductorResult<()> {
        self.0.read().await.check_running()
    }

    async fn install_dna(&self) -> ConductorResult<()> {
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
        cell.invoke_zome(api, invocation)
            .await
            .map_err(Into::into)
    }
}

impl<DS: DnaStore> ConductorHandleImpl<DS> {
    /// Creates new handle
    pub fn new(conductor: Conductor<DS>) -> Self {
        let conductor_handle: Arc<RwLock<Box<Conductor<DS>>>> =
            Arc::new(RwLock::new(Box::new(conductor)));
        ConductorHandleImpl(conductor_handle)
    }
}

pub type ConductorHandle = Box<dyn ConductorHandleT + Send>;
