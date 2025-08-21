//! For more details see [`holochain_integrity_types::op`].

use crate::prelude::{Deserialize, Serialize};
use holo_hash::ActionHash;
use holochain_integrity_types::Action;

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

impl ChainOpType {
    /// Calculate the op's sys validation dependencies (action hashes)
    pub fn sys_validation_dependencies(&self, action: &Action) -> Vec<ActionHash> {
        match self {
            ChainOpType::StoreRecord | ChainOpType::StoreEntry => vec![],
            ChainOpType::RegisterAgentActivity => action
                .prev_action()
                .map(|p| vec![p.clone()])
                .unwrap_or_default(),
            ChainOpType::RegisterUpdatedContent | ChainOpType::RegisterUpdatedRecord => {
                match action {
                    Action::Update(update) => vec![update.original_action_address.clone()],
                    _ => vec![],
                }
            }
            ChainOpType::RegisterDeletedBy | ChainOpType::RegisterDeletedEntryAction => {
                match action {
                    Action::Delete(delete) => vec![delete.deletes_address.clone()],
                    _ => vec![],
                }
            }
            ChainOpType::RegisterAddLink => vec![],
            ChainOpType::RegisterRemoveLink => match action {
                Action::DeleteLink(delete_link) => vec![delete_link.link_add_address.clone()],
                _ => vec![],
            },
        }
    }
}

#[cfg(any(feature = "sqlite", feature = "sqlite-encrypted"))]
impl rusqlite::ToSql for ChainOpType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            format!("{}", self).into(),
        ))
    }
}

#[cfg(any(feature = "sqlite", feature = "sqlite-encrypted"))]
impl rusqlite::types::FromSql for ChainOpType {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        use std::str::FromStr;

        String::column_result(value).and_then(|string| {
            ChainOpType::from_str(&string).map_err(|_| rusqlite::types::FromSqlError::InvalidType)
        })
    }
}
