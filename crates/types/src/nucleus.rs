//! Types related to making calls into Zomes.
use holochain_zome_types::zome::ZomeName;
use crate::{cell::CellId};

/// The ZomeId is a pair of CellId and ZomeName.
pub type ZomeId = (CellId, ZomeName);
