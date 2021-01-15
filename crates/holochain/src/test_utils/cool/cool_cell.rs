use super::{CoolConductor, CoolZome};
use hdk3::prelude::*;
use holo_hash::DnaHash;

/// A reference to a Cell created by a CoolConductor installation function.
/// It has very concise methods for calling a zome on this cell
#[derive(Clone, derive_more::Constructor)]
pub struct CoolCell {
    pub(super) cell_id: CellId,
    pub(super) handle: CoolConductor,
}

impl CoolCell {
    /// Accessor for CellId
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Accessor for AgentPubKey
    pub fn agent_pubkey(&self) -> &AgentPubKey {
        &self.cell_id.agent_pubkey()
    }

    /// Accessor for DnaHash
    pub fn dna_hash(&self) -> &DnaHash {
        &self.cell_id.dna_hash()
    }

    /// Get a CoolZome with the given name
    pub fn zome<Z: Into<ZomeName>>(&self, zome_name: Z) -> CoolZome {
        CoolZome::new(
            self.cell_id().clone(),
            zome_name.into(),
            self.handle.clone(),
        )
    }

    /// Call a zome function on this CoolCell as if from another Agent.
    /// The provenance and optional CapSecret must be provided.
    pub async fn call_from<I, O, Z, F>(
        &self,
        provenance: AgentPubKey,
        cap: Option<CapSecret>,
        zome_name: Z,
        fn_name: F,
        payload: I,
    ) -> O
    where
        ZomeName: From<Z>,
        FunctionName: From<F>,
        I: serde::Serialize,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.handle
            .call_zome_ok_flat(
                self.cell_id(),
                zome_name,
                fn_name,
                cap,
                Some(provenance),
                payload,
            )
            .await
    }

    /// Call a zome function on this CoolCell.
    /// No CapGrant is provided, since the authorship capability will be granted.
    pub async fn call<I, O, Z, F>(&self, zome_name: Z, fn_name: F, payload: I) -> O
    where
        ZomeName: From<Z>,
        FunctionName: From<F>,
        I: serde::Serialize,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_from(
            self.agent_pubkey().clone(),
            None,
            zome_name,
            fn_name,
            payload,
        )
        .await
    }
}
