//! A wrapper around ConductorHandle with more convenient methods

use crate::{conductor::handle::ConductorHandle, core::ribosome::ZomeCallInvocation};
use hdk3::prelude::*;
use holochain_types::dna::zome::Zome;
use unwrap_to::unwrap_to;

/// A wrapper around ConductorHandle with more convenient methods
#[derive(shrinkwraprs::Shrinkwrap, derive_more::From)]
pub struct TestConductorHandle(ConductorHandle);

impl TestConductorHandle {
    /// Call a zome function with automatic de/serialization
    pub async fn call<I, O, F, E>(
        &self,
        cell_id: &CellId,
        zome: &Zome,
        fn_name: F,
        cap: Option<CapSecret>,
        provenance: Option<AgentPubKey>,
        payload: I,
    ) -> O
    where
        E: std::fmt::Debug,
        FunctionName: From<F>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E> + std::fmt::Debug,
    {
        let payload = ExternInput::new(payload.try_into().expect("Couldn't serialize payload"));
        let provenance = provenance.unwrap_or_else(|| cell_id.agent_pubkey().clone());
        let invocation = ZomeCallInvocation {
            cell_id: cell_id.clone(),
            zome: zome.clone(),
            fn_name: fn_name.into(),
            cap,
            provenance,
            payload,
        };
        let response = self.call_zome(invocation).await.unwrap().unwrap();
        unwrap_to!(response => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .expect("Couldn't deserialize zome call output")
    }
}
