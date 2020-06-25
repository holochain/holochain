//! Types related to making calls into Zomes.
use crate::cell::CellId;
use holochain_zome_types::zome::ZomeName;

/// The ZomePosition is a pair of CellId and ZomeName.
pub type ZomePosition = (CellId, ZomeName);
