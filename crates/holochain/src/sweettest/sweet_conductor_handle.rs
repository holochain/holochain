use super::SweetZome;
use crate::conductor::{api::error::ConductorApiResult, ConductorHandle};
use holochain_conductor_api::ZomeCall;
use holochain_types::prelude::*;
use unwrap_to::unwrap_to;
use crate::conductor::api::error::ConductorApiError;

/// A wrapper around ConductorHandle with more convenient methods for testing
/// and a cleanup drop
#[derive(shrinkwraprs::Shrinkwrap, derive_more::From)]
pub struct SweetConductorHandle(pub(crate) ConductorHandle);

impl SweetConductorHandle {
    /// Handle accessor.
    pub fn handle(&self) -> ConductorHandle {
        std::sync::Arc::clone(&self.0)
    }

    /// Make a zome call to a Cell, as if that Cell were the caller. Most common case.
    /// No capability is necessary, since the authorship capability is automatically granted.
    pub async fn call<I, O, F>(&self, zome: &SweetZome, fn_name: F, payload: I) -> O
    where
        FunctionName: From<F>,
        I: serde::Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_fallible(zome, fn_name, payload).await.unwrap()
    }

    /// Like `call`, but without the unwrap
    pub async fn call_fallible<I, O, F>(
        &self,
        zome: &SweetZome,
        fn_name: F,
        payload: I,
    ) -> ConductorApiResult<O>
    where
        FunctionName: From<F>,
        I: serde::Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_from_fallible(zome.cell_id().agent_pubkey(), None, zome, fn_name, payload)
            .await
    }
    /// Make a zome call to a Cell, as if some other Cell were the caller. More general case.
    /// Can optionally provide a capability.
    pub async fn call_from<I, O, F>(
        &self,
        provenance: &AgentPubKey,
        cap_secret: Option<CapSecret>,
        zome: &SweetZome,
        fn_name: F,
        payload: I,
    ) -> O
    where
        FunctionName: From<F>,
        I: Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_from_fallible(provenance, cap_secret, zome, fn_name, payload)
            .await
            .unwrap()
    }

    /// Like `call_from`, but without the unwrap
    pub async fn call_from_fallible<I, O, F>(
        &self,
        provenance: &AgentPubKey,
        cap_secret: Option<CapSecret>,
        zome: &SweetZome,
        fn_name: F,
        payload: I,
    ) -> ConductorApiResult<O>
    where
        FunctionName: From<F>,
        I: Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        let payload = ExternIO::encode(payload).expect("Couldn't serialize payload");
        let call = ZomeCall {
            cell_id: zome.cell_id().clone(),
            zome_name: zome.name().clone(),
            fn_name: fn_name.into(),
            cap_secret,
            provenance: provenance.clone(),
            payload,
        };
        match self.handle().call_zome(call).await {
            Ok(Ok(response)) => Ok(unwrap_to!(response => ZomeCallResponse::Ok).decode().expect("Couldn't deserialize zome call output")),
            Ok(Err(error)) => Err(ConductorApiError::Other(Box::new(error))),
            Err(error) => Err(error),
        }
    }

    // /// Get a stream of all Signals emitted since the time of this function call.
    // pub async fn signal_stream(&self) -> impl tokio_stream::Stream<Item = Signal> {
    //     self.0.signal_broadcaster().await.subscribe_merged()
    // }

    /// Manually await shutting down the conductor.
    /// Conductors are already cleaned up on drop but this
    /// is useful if you need to know when it's finished cleaning up.
    pub async fn shutdown_and_wait(&self) {
        let c = &self.0;
        if let Some(shutdown) = c.take_shutdown_handle() {
            c.shutdown();
            shutdown
                .await
                .expect("Failed to await shutdown handle")
                .expect("Conductor shutdown error");
        }
    }

    /// Intentionally private clone function, only to be used internally
    pub(super) fn clone_privately(&self) -> Self {
        Self(self.0.clone())
    }
}
