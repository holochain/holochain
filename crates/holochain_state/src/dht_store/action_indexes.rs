//! Shared dispatch for populating the per-action index tables
//! (`Link`, `DeletedLink`, `UpdatedRecord`, `DeletedRecord`).
//!
//! Both the source-chain authored-data writer and the cache writer convert
//! incoming actions to the [`ActionData`] form and then need to insert
//! into the same index tables based on the action variant. This helper
//! holds the single dispatch.

use holo_hash::ActionHash;
use holochain_data::dht::{
    InsertDeletedLink, InsertDeletedRecord, InsertLink, InsertUpdatedRecord,
};
use holochain_data::kind::Dht;
use holochain_data::TxWrite;
use holochain_zome_types::prelude::ActionData;

use crate::mutations::{StateMutationError, StateMutationResult};

/// Insert into the appropriate index table for the given action variant.
///
/// `CreateLink`, `DeleteLink`, `Update`, and `Delete` populate their
/// respective indices; all other variants are no-ops.
pub(crate) async fn insert_action_indexes(
    tx: &mut TxWrite<Dht>,
    action_hash: &ActionHash,
    action_data: &ActionData,
) -> StateMutationResult<()> {
    match action_data {
        ActionData::CreateLink(a) => {
            tx.insert_link_index(InsertLink {
                action_hash,
                base_hash: &a.base_address,
                zome_index: a.zome_index.0,
                link_type: a.link_type.0,
                tag: Some(a.tag.0.as_slice()),
            })
            .await
            .map_err(StateMutationError::from)?;
        }
        ActionData::DeleteLink(a) => {
            tx.insert_deleted_link_index(InsertDeletedLink {
                action_hash,
                create_link_hash: &a.link_add_address,
            })
            .await
            .map_err(StateMutationError::from)?;
        }
        ActionData::Update(a) => {
            tx.insert_updated_record_index(InsertUpdatedRecord {
                action_hash,
                original_action_hash: &a.original_action_address,
                original_entry_hash: &a.original_entry_address,
            })
            .await
            .map_err(StateMutationError::from)?;
        }
        ActionData::Delete(a) => {
            tx.insert_deleted_record_index(InsertDeletedRecord {
                action_hash,
                deletes_action_hash: &a.deletes_address,
                deletes_entry_hash: &a.deletes_entry_address,
            })
            .await
            .map_err(StateMutationError::from)?;
        }
        _ => {}
    }
    Ok(())
}
