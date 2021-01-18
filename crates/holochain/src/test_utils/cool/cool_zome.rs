use hdk3::prelude::*;

/// A reference to a Zome in a Cell created by a CoolConductor installation function.
/// Think of it as a partially applied CoolCell, with the ZomeName baked in.
#[derive(Clone, derive_more::Constructor)]
pub struct CoolZome {
    cell_id: CellId,
    name: ZomeName,
}

impl CoolZome {
    // /// Call a function as if from another Agent.
    // /// The provenance and optional CapSecret must be provided.
    // pub async fn call_from<I, O, F>(
    //     &self,
    //     provenance: AgentPubKey,
    //     cap: Option<CapSecret>,
    //     fn_name: F,
    //     payload: I,
    // ) -> O
    // where
    //     FunctionName: From<F>,
    //     I: serde::Serialize,
    //     O: serde::de::DeserializeOwned + std::fmt::Debug,
    // {
    //     self.handle
    //         .call_zome_ok_flat(
    //             &self.cell_id,
    //             self.zome_name.clone(),
    //             fn_name,
    //             cap,
    //             Some(provenance),
    //             payload,
    //         )
    //         .await
    // }

    /// Call a function on this zome.
    /// No CapGrant is provided; the authorship capability will be granted.
    pub async fn call<I, O, F>(&self, fn_name: F, payload: I) -> O
    where
        FunctionName: From<F>,
        I: serde::Serialize,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_from(self.cell_id.agent_pubkey().clone(), None, fn_name, payload)
            .await
    }

    /// Accessor
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Accessor
    pub fn name(&self) -> &ZomeName {
        &self.name
    }
}
