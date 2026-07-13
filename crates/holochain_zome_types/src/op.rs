//! For more details see [`holochain_integrity_types::op`].

use crate::prelude::{Deserialize, Serialize};

#[doc(no_inline)]
pub use holochain_integrity_types::op;
#[doc(inline)]
pub use holochain_integrity_types::op::*;

/// This enum is used to encode just the enum variant of ChainOp
#[allow(missing_docs)]
#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    derive_more::Display,
    strum_macros::EnumString,
)]
pub enum ChainOpType {
    #[display("StoreRecord")]
    StoreRecord,
    #[display("StoreEntry")]
    StoreEntry,
    #[display("RegisterAgentActivity")]
    RegisterAgentActivity,
    #[display("RegisterUpdatedContent")]
    RegisterUpdatedContent,
    #[display("RegisterUpdatedRecord")]
    RegisterUpdatedRecord,
    #[display("RegisterDeletedBy")]
    RegisterDeletedBy,
    #[display("RegisterDeletedEntryAction")]
    RegisterDeletedEntryAction,
    #[display("RegisterAddLink")]
    RegisterAddLink,
    #[display("RegisterRemoveLink")]
    RegisterRemoveLink,
}
