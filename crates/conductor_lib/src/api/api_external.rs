use crate::conductor::Conductor;
use std::sync::Arc;
use sx_conductor_api::{AdminMethod, ConductorApiResult, ExternalConductorApiT};
use sx_types::{
    cell::CellId,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    prelude::*,
};
use tokio::sync::RwLock;

/// The interface that a Conductor exposes to the outside world.
/// The Conductor lives inside an Arc<RwLock<_>> for the benefit of
/// all other API handles
pub struct ExternalConductorApi {
    conductor_mutex: Arc<RwLock<Conductor>>,
}

impl ExternalConductorApi {
    pub fn new(conductor_mutex: Arc<RwLock<Conductor>>) -> Self {
        Self { conductor_mutex }
    }
}

#[async_trait::async_trait]
impl ExternalConductorApiT for ExternalConductorApi {
    async fn admin(&mut self, _method: AdminMethod) -> ConductorApiResult<JsonString> {
        unimplemented!()
    }

    async fn invoke_zome(
        &self,
        _cell_id: &CellId,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_mutex.read().await;
        unimplemented!()
    }
}
