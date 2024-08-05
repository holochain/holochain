use super::SweetZome;
use crate::conductor::{api::error::ConductorApiResult, ConductorHandle};
use holochain_types::prelude::*;

/// A wrapper around ConductorHandle with more convenient methods for testing
/// and a cleanup drop
#[derive(shrinkwraprs::Shrinkwrap, derive_more::From)]
pub struct SweetConductorHandle(pub(crate) ConductorHandle);

impl SweetConductorHandle {
    /// Make a zome call to a Cell, as if that Cell were the caller. Most common case.
    /// No capability is necessary, since the authorship capability is automatically granted.
    pub async fn call<I, O>(
        &self,
        zome: &SweetZome,
        fn_name: impl Into<FunctionName>,
        payload: I,
    ) -> O
    where
        I: serde::Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_fallible(zome, fn_name, payload).await.unwrap()
    }

    /// Like `call`, but without the unwrap
    pub async fn call_fallible<I, O>(
        &self,
        zome: &SweetZome,
        fn_name: impl Into<FunctionName>,
        payload: I,
    ) -> ConductorApiResult<O>
    where
        I: serde::Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_from_fallible(zome.cell_id().agent_pubkey(), None, zome, fn_name, payload)
            .await
    }
    /// Make a zome call to a Cell, as if some other Cell were the caller. More general case.
    /// Can optionally provide a capability.
    pub async fn call_from<I, O>(
        &self,
        provenance: &AgentPubKey,
        cap_secret: Option<CapSecret>,
        zome: &SweetZome,
        fn_name: impl Into<FunctionName>,
        payload: I,
    ) -> O
    where
        I: Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_from_fallible(provenance, cap_secret, zome, fn_name, payload)
            .await
            .unwrap()
    }

    /// Like `call_from`, but without the unwrap
    pub async fn call_from_fallible<I, O>(
        &self,
        provenance: &AgentPubKey,
        cap_secret: Option<CapSecret>,
        zome: &SweetZome,
        fn_name: impl Into<FunctionName>,
        payload: I,
    ) -> ConductorApiResult<O>
    where
        I: Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.0
            .easy_call_zome(
                provenance,
                cap_secret,
                zome.cell_id().clone(),
                zome.name().clone(),
                fn_name,
                payload,
            )
            .await
    }

    /// Intentionally private clone function, only to be used internally
    pub(super) fn clone_privately(&self) -> Self {
        Self(self.0.clone())
    }
}
