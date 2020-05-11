//! Types related to making calls into Zomes.
use crate::cell::CellId;
use holochain_zome_types::zome::ZomeName;

/// The ZomeId is a pair of CellId and ZomeName.
pub type ZomeId = (CellId, ZomeName);
