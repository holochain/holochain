use super::{TestConductorHandle, TestZome};
use hdk3::prelude::*;
use holo_hash::DnaHash;

/// A reference to a Cell created by a TestConductorHandle installation function.
/// It has very concise methods for calling a zome on this cell
#[derive(Clone, derive_more::Constructor)]
pub struct TestCell {
    pub(super) cell_id: CellId,
    pub(super) handle: TestConductorHandle,
}

impl TestCell {
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

    /// Get a TestZome with the given name
    pub fn zome<Z: Into<ZomeName>>(&self, zome_name: Z) -> TestZome {
        TestZome::new(
            self.cell_id().clone(),
            zome_name.into(),
            self.handle.clone(),
        )
    }

    /// Call a zome function on this TestCell as if from another Agent.
    /// The provenance and optional CapSecret must be provided.
    pub async fn call_from<I, O, Z, F, E>(
        &self,
        provenance: AgentPubKey,
        cap: Option<CapSecret>,
        zome_name: Z,
        fn_name: F,
        payload: I,
    ) -> O
    where
        E: std::fmt::Debug,
        ZomeName: From<Z>,
        FunctionName: From<F>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E> + std::fmt::Debug,
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

    /// Call a zome function on this TestCell.
    /// No CapGrant is provided, since the authorship capability will be granted.
    pub async fn call<I, O, Z, F, E>(&self, zome_name: Z, fn_name: F, payload: I) -> O
    where
        E: std::fmt::Debug,
        ZomeName: From<Z>,
        FunctionName: From<F>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E> + std::fmt::Debug,
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
