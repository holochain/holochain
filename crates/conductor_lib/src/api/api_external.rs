use crate::conductor::Conductor;
use std::sync::Arc;
use sx_conductor_api::{
    AdminMethod, CellConductorInterfaceT, ConductorApiResult, ExternalConductorInterfaceT,
};
use sx_types::{nucleus::ZomeInvocation, prelude::*, shims::*};
use tokio::sync::{RwLock, RwLockWriteGuard};
use super::CellConductorInterface;

// #[derive(Clone)]
pub struct ExternalConductorInterface<Api: CellConductorInterfaceT = CellConductorInterface> {
    conductor_mutex: Arc<RwLock<Api::Conductor>>,
}

impl<Api: CellConductorInterfaceT> ExternalConductorInterface<Api> {
    pub fn new(conductor_mutex: Arc<RwLock<Api::Conductor>>) -> Self {
        Self { conductor_mutex }
    }
}

#[async_trait::async_trait]
impl<Api: CellConductorInterfaceT> ExternalConductorInterfaceT for ExternalConductorInterface<Api>
// where
//     Api::Conductor: std::marker::Send,
//     Api::Conductor: std::marker::Sync,
//     Api::Conductor: std::clone::Clone
{
    type Conductor = <Api as CellConductorInterfaceT>::Conductor;

    // async fn conductor_mut(&self) -> RwLockWriteGuard<'_, Self::Conductor> {
    //     self.conductor_mutex.write().await
    // }

    async fn admin(&mut self, _method: AdminMethod) -> ConductorApiResult<JsonString> {
        unimplemented!()
    }
}
