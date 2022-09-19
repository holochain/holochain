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

#[deprecated = "alias for SweetInlineZomes"]
/// Alias for SweetInlineZomes
pub type SweetEasyInline = SweetInlineZomes;

#[derive(Default)]
/// A helper for creating [`InlineZomeSet`]
pub struct SweetInlineZomes(pub InlineZomeSet);

impl SweetInlineZomes {
    /// Zome name for the integrity zome.
    pub const INTEGRITY: &'static str = "integrity";
    /// Zome name for the coordinator zome.
    pub const COORDINATOR: &'static str = "coordinator";

    /// Create a single integrity zome with the [`ZomeName`] "integrity"
    /// and coordinator zome with the [`ZomeName`] Coordinator.
    pub fn new(entry_defs: Vec<EntryDef>, num_link_types: u8) -> Self {
        Self(
            InlineZomeSet::new_unique(
                [(Self::INTEGRITY, entry_defs, num_link_types)],
                [Self::COORDINATOR],
            )
            .with_dependency(Self::COORDINATOR, Self::INTEGRITY),
        )
    }

    /// Add a function to the integrity zome.
    pub fn integrity_function<F, I, O>(self, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        Self(self.0.function(Self::INTEGRITY, name, f))
    }

    /// Add a function to the coordinator_zome.
    pub fn function<F, I, O>(self, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        Self(self.0.function(Self::COORDINATOR, name, f))
    }

    /// Alias for `integrity_function`
    #[deprecated = "Alias for `integrity_function`"]
    pub fn integrity_callback<F, I, O>(self, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        self.integrity_function(name, f)
    }

    /// Alias for `function`
    #[deprecated = "Alias for `function`"]
    pub fn callback<F, I, O>(self, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        self.function(name, f)
    }
}

impl From<SweetInlineZomes> for InlineZomeSet {
    fn from(s: SweetInlineZomes) -> Self {
        s.0
    }
}
