use hdk::prelude::*;
use holochain_types::inline_zome::InlineZomeSet;
use serde::de::DeserializeOwned;

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

#[derive(Default)]
/// A helper for creating [`InlineZomeSet`]
pub struct SweetEasyInline(pub InlineZomeSet);

impl SweetEasyInline {
    /// Zome name for the integrity zome.
    pub const INTEGRITY: &'static str = "integrity";
    /// Zome name for the coordinator zome.
    pub const COORDINATOR: &'static str = "coordinator";

    /// Create a single integrity zome with the [`ZomeName`] "integrity"
    /// and coordinator zome with the [`ZomeName`] Coordinator.
    pub fn new(entry_defs: Vec<EntryDef>) -> Self {
        Self(InlineZomeSet::new_unique(
            [(Self::INTEGRITY, entry_defs)],
            [Self::COORDINATOR],
        ))
    }

    /// Add a callback to the integrity zome.
    pub fn integrity_callback<F, I, O>(self, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        Self(self.0.callback(Self::INTEGRITY, name, f))
    }

    /// Add a callback to the coordinator_zome.
    pub fn callback<F, I, O>(self, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        Self(self.0.callback(Self::COORDINATOR, name, f))
    }
}

impl From<SweetEasyInline> for InlineZomeSet {
    fn from(s: SweetEasyInline) -> Self {
        s.0
    }
}
