use super::{
    api::{error::ConductorApiResult, CellConductorApi},
    config::AdminInterfaceConfig,
    dna_store::DnaStore,
    error::ConductorResult,
    manager::TaskManagerRunHandle,
    Cell, Conductor,
};
use derive_more::From;
use std::sync::Arc;
use holochain_types::{
    dna::Dna,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    prelude::*,
};
use tokio::sync::RwLock;

pub type ConductorHandle = Arc<dyn ConductorHandleT>;
pub type ConductorHandleInner<DS> = RwLock<Conductor<DS>>;

#[async_trait::async_trait]
pub trait ConductorHandleT: Send + Sync {
    async fn check_running(&self) -> ConductorResult<()>;
    async fn add_admin_interfaces_via_handle(
        &self,
        handle: ConductorHandle,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()>;
    async fn install_dna(&self, dna: Dna) -> ConductorResult<()>;
    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>>;
    async fn invoke_zome(
        &self,
        api: CellConductorApi,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

    async fn get_wait_handle(&self) -> Option<TaskManagerRunHandle>;
    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16>;
    async fn shutdown(&self);
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

    async fn add_admin_interfaces_via_handle(
        &self,
        handle: ConductorHandle,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()> {
        let mut lock = self.0.write().await;
        lock.add_admin_interfaces_via_handle(handle, configs).await
    }

    async fn install_dna(&self, dna: Dna) -> ConductorResult<()> {
        Ok(self.0.write().await.dna_store_mut().add(dna)?)
    }

    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>> {
        Ok(self.0.read().await.dna_store().list())
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

    async fn get_wait_handle(&self) -> Option<TaskManagerRunHandle> {
        self.0.write().await.get_wait_handle()
    }

    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        self.0.read().await.get_arbitrary_admin_websocket_port()
    }

    async fn shutdown(&self) {
        self.0.write().await.shutdown()
    }
}
