use super::CoolConductor;
use hdk3::prelude::*;

/// A reference to a Zome in a Cell created by a CoolConductor installation function.
/// Think of it as a partially applied CoolCell, with the ZomeName baked in.
#[derive(Clone, derive_more::Constructor)]
pub struct CoolZome {
    cell_id: CellId,
    zome_name: ZomeName,
    handle: CoolConductor,
}

impl CoolZome {
    /// Call a function as if from another Agent.
    /// The provenance and optional CapSecret must be provided.
    pub async fn call_from<I, O, F, E>(
        &self,
        provenance: AgentPubKey,
        cap: Option<CapSecret>,
        fn_name: F,
        payload: I,
    ) -> O
    where
        E: std::fmt::Debug,
        FunctionName: From<F>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E> + std::fmt::Debug,
    {
        self.handle
            .call_zome_ok_flat(
                &self.cell_id,
                self.zome_name.clone(),
                fn_name,
                cap,
                Some(provenance),
                payload,
            )
            .await
    }

    /// Call a function on this zome.
    /// No CapGrant is provided; the authorship capability will be granted.
    pub async fn call<I, O, F, E>(&self, fn_name: F, payload: I) -> O
    where
        E: std::fmt::Debug,
        FunctionName: From<F>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E> + std::fmt::Debug,
    {
        self.call_from(self.cell_id.agent_pubkey().clone(), None, fn_name, payload)
            .await
    }
}
