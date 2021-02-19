use hdk::prelude::*;

/// A reference to a Zome in a Cell created by a SweetConductor installation function.
/// Think of it as a partially applied SweetCell, with the ZomeName baked in.
#[derive(Clone, derive_more::Constructor)]
pub struct SweetZome {
    cell_id: CellId,
    name: ZomeName,
}

impl SweetZome {
    /// Accessor
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Accessor
    pub fn name(&self) -> &ZomeName {
        &self.name
    }
}
