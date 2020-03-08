use crate::conductor::Conductor;
use sx_conductor_api::{AdminMethod, ExternalConductorInterfaceT};

use std::sync::Arc;
use sx_cell::cell::Cell;
use sx_conductor_api::{CellConductorInterfaceT, ConductorApiResult};
use sx_types::{nucleus::ZomeInvocation, prelude::*, shims::*};
use tokio::sync::{RwLock, RwLockWriteGuard};

#[derive(Clone)]
pub struct ExternalConductorInterface<Api: CellConductorInterfaceT> {
    conductor_mutex: Arc<RwLock<Api::Conductor>>,
}

impl<Api: CellConductorInterfaceT> ExternalConductorInterface<Api> {
    pub fn new(conductor_mutex: Arc<RwLock<Conductor>>) -> Self {
        Self { conductor_mutex }
    }
}

#[async_trait::async_trait(?Send)]
impl<Api: CellConductorInterfaceT> ExternalConductorInterfaceT for ExternalConductorInterface<Api>
where
    Api::Conductor: std::marker::Send,
    Api::Conductor: std::marker::Sync,
    Api::Conductor: std::clone::Clone
{
    type Conductor = Api::Conductor;

    async fn conductor_mut(&self) -> RwLockWriteGuard<'_, Self::Conductor> {
        self.conductor_mutex.write().await
    }

    async fn admin(&mut self, _method: AdminMethod) -> ConductorApiResult<JsonString> {
        unimplemented!()
    }
}
