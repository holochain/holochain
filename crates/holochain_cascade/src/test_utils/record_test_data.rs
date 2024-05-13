use holo_hash::ActionHash;
use holo_hash::EntryHash;
use holochain_serialized_bytes::SerializedBytes;
use holochain_serialized_bytes::UnsafeBytes;
use holochain_state::prelude::*;
use std::convert::TryInto;

use ::fixt::prelude::*;

/// A collection of fixtures for use in Cascade tests
#[derive(Debug)]
#[allow(missing_docs)]
pub struct RecordTestData {
    pub store_record_op: ChainOpHashed,
    pub wire_create: Judged<SignedAction>,
    pub create_hash: ActionHash,
    pub deleted_by_op: ChainOpHashed,
    pub wire_delete: Judged<WireDelete>,
    pub delete_hash: ActionHash,
    pub update_record_op: ChainOpHashed,
    pub wire_update: Judged<WireUpdateRelationship>,
    pub update_hash: ActionHash,
    pub hash: EntryHash,
    pub entry: Entry,
    /// An Op from any_record
    pub any_store_record_op: ChainOpHashed,
    /// An arbitrary Action
    pub any_action: Judged<SignedAction>,
    /// The hash of any_action
    pub any_action_hash: ActionHash,
    /// The entry to go with any_action
    pub any_entry: Option<Entry>,
    /// The hash of any_entry
    pub any_entry_hash: Option<EntryHash>,
    /// A Record constructed from any_action
    pub any_record: Record,
}

impl RecordTestData {
    /// Constructor
    pub fn create() -> Self {
        let mut create = fixt!(Create);
        let mut update = fixt!(Update);
        let mut delete = fixt!(Delete);
        let mut any_action = fixt!(Action);
        let entry: AppEntryBytes = SerializedBytes::from(UnsafeBytes::from(vec![0u8]))
            .try_into()
            .unwrap();
        let entry = Entry::App(entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        let update_entry = fixt!(AppEntryBytes);
        let update_entry = Entry::App(update_entry);
        let update_entry_hash = EntryHash::with_data_sync(&update_entry);

        let mut entry_type_fixt =
            AppEntryDefFixturator::new(EntryVisibility::Public).map(EntryType::App);

        create.entry_hash = entry_hash.clone();
        create.entry_type = entry_type_fixt.next().unwrap();
        update.entry_hash = update_entry_hash;
        update.entry_type = entry_type_fixt.next().unwrap();

        let create_action = Action::Create(create);
        let create_hash = ActionHash::with_data_sync(&create_action);

        delete.deletes_address = create_hash.clone();
        delete.deletes_entry_address = entry_hash.clone();

        update.original_entry_address = entry_hash.clone();
        update.original_action_address = create_hash.clone();

        let delete_action = Action::Delete(delete.clone());
        let update_action = Action::Update(update.clone());
        let delete_hash = ActionHash::with_data_sync(&delete_action);
        let update_hash = ActionHash::with_data_sync(&update_action);

        let signature = fixt!(Signature);
        let store_record_op = ChainOpHashed::from_content_sync(ChainOp::StoreRecord(
            signature.clone(),
            create_action.clone(),
            entry.clone().into(),
        ));

        let wire_create = Judged::valid(SignedAction(create_action, signature));

        let signature = fixt!(Signature);
        let deleted_by_op =
            ChainOpHashed::from_content_sync(ChainOp::RegisterDeletedBy(signature.clone(), delete));

        let wire_delete = Judged::valid(SignedAction(delete_action, signature).try_into().unwrap());

        let signature = fixt!(Signature);
        let update_record_op = ChainOpHashed::from_content_sync(ChainOp::RegisterUpdatedRecord(
            signature.clone(),
            update,
            update_entry.into(),
        ));
        let wire_update = Judged::valid(SignedAction(update_action, signature).try_into().unwrap());

        let mut any_entry = None;
        let mut any_entry_hash = None;
        if any_action.entry_hash().is_some() {
            match &mut any_action {
                Action::Create(Create {
                    entry_hash: eh,
                    entry_type,
                    ..
                })
                | Action::Update(Update {
                    entry_hash: eh,
                    entry_type,
                    ..
                }) => {
                    let entry: AppEntryBytes = SerializedBytes::from(UnsafeBytes::from(vec![1u8]))
                        .try_into()
                        .unwrap();
                    let entry = Entry::App(entry);
                    *entry_type = entry_type_fixt.next().unwrap();
                    *eh = EntryHash::with_data_sync(&entry);
                    any_entry_hash = Some(eh.clone());
                    any_entry = Some(entry);
                }
                _ => unreachable!(),
            }
        }

        let any_action_hash = ActionHash::with_data_sync(&any_action);

        let signature = fixt!(Signature);
        let any_store_record_op = ChainOpHashed::from_content_sync(ChainOp::StoreRecord(
            signature.clone(),
            any_action.clone(),
            RecordEntry::new(any_action.entry_visibility(), any_entry.clone()),
        ));

        let any_record = Record::new(
            SignedActionHashed::with_presigned(
                ActionHashed::from_content_sync(any_action.clone()),
                signature.clone(),
            ),
            any_entry.clone(),
        );

        let any_action = Judged::valid(SignedAction(any_action, signature));

        Self {
            store_record_op,
            deleted_by_op,
            update_record_op,
            hash: entry_hash,
            entry,
            wire_create,
            wire_delete,
            wire_update,
            create_hash,
            delete_hash,
            update_hash,
            any_store_record_op,
            any_action,
            any_action_hash,
            any_entry,
            any_entry_hash,
            any_record,
        }
    }
}
