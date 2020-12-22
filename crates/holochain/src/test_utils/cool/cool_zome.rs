use hdk3::prelude::*;

/// A reference to a Zome in a Cell created by a CoolConductor installation function.
/// Think of it as a partially applied CoolCell, with the ZomeName baked in.
#[derive(Clone, derive_more::Constructor)]
pub struct CoolZome {
    cell_id: CellId,
    name: ZomeName,
}

impl CoolZome {
    /// Accessor
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Accessor
    pub fn name(&self) -> &ZomeName {
        &self.name
    }
}
