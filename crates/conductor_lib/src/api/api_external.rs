use crate::conductor::Conductor;
use std::sync::Arc;
use sx_conductor_api::{
    AdminMethod, CellConductorInterfaceT, ConductorApiResult, ExternalConductorInterfaceT,
};
use sx_types::{nucleus::{ZomeInvocationResponse, ZomeInvocation}, prelude::*, shims::*, agent::CellId};
use tokio::sync::{RwLock, RwLockWriteGuard};
use super::CellConductorInterface;

// #[derive(Clone)]
pub struct ExternalConductorInterface {
    conductor_mutex: Arc<RwLock<Conductor>>,
}

impl ExternalConductorInterface {
    pub fn new(conductor_mutex: Arc<RwLock<Conductor>>) -> Self {
        Self { conductor_mutex }
    }
}

#[async_trait::async_trait]
impl ExternalConductorInterfaceT for ExternalConductorInterface
{
    async fn admin(&mut self, _method: AdminMethod) -> ConductorApiResult<JsonString> {
        unimplemented!()
    }

    async fn invoke_zome(
        &self,
        cell_id: &CellId,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        unimplemented!()
    }

}
