use holo_hash::ActionHash;
use holo_hash::EntryHash;
use holochain_serialized_bytes::UnsafeBytes;
use holochain_state::prelude::*;
use std::convert::TryInto;

use ::fixt::prelude::*;

/// A collection of test fixtures used to test entry-related cascade functionality
#[derive(Debug)]
#[allow(missing_docs)]
pub struct EntryTestData {
    pub store_entry_op: ChainOpHashed,
    pub wire_create: Judged<WireNewEntryAction>,
    pub create_hash: ActionHash,
    pub delete_entry_action_op: ChainOpHashed,
    pub wire_delete: Judged<WireDelete>,
    pub delete_hash: ActionHash,
    pub update_content_op: ChainOpHashed,
    pub wire_update: Judged<WireUpdateRelationship>,
    pub update_hash: ActionHash,
    pub hash: EntryHash,
    pub entry: EntryData,
    // Links
    pub create_link_op: ChainOpHashed,
    pub delete_link_op: ChainOpHashed,
    pub wire_create_link: WireCreateLink,
    pub wire_create_link_base: WireCreateLink,
    pub wire_delete_link: WireDeleteLink,
    pub create_link_action: SignedActionHashed,
    pub delete_link_action: SignedActionHashed,
    pub link_key: WireLinkKey,
    pub link_key_tag: WireLinkKey,
    pub links: Vec<Link>,
    pub link_query: WireLinkQuery,
}

impl EntryTestData {
    /// Create the test fixtures
    pub fn create() -> Self {
        let mut create = fixt!(Create);
        let mut update = fixt!(Update);
        let mut delete = fixt!(Delete);

        let mut create_link = fixt!(CreateLink);
        create_link.zome_index = 0.into();
        let mut delete_link = fixt!(DeleteLink);

        let entry: AppEntryBytes = SerializedBytes::from(UnsafeBytes::from(vec![3u8]))
            .try_into()
            .unwrap();
        let entry = Entry::App(entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        let update_entry: AppEntryBytes = SerializedBytes::from(UnsafeBytes::from(vec![4u8]))
            .try_into()
            .unwrap();
        let update_entry = Entry::App(update_entry);
        let update_entry_hash = EntryHash::with_data_sync(&update_entry);

        let mut entry_type_fixt =
            AppEntryDefFixturator::new(EntryVisibility::Public).map(EntryType::App);

        create.entry_hash = entry_hash.clone();
        create.entry_type = entry_type_fixt.next().unwrap();
        update.entry_hash = update_entry_hash;
        update.entry_type = entry_type_fixt.next().unwrap();

        let create_action = Action::Create(create.clone());
        let create_hash = ActionHash::with_data_sync(&create_action);

        delete.deletes_entry_address = entry_hash.clone();
        delete.deletes_address = create_hash.clone();

        update.original_entry_address = entry_hash.clone();
        update.original_action_address = create_hash.clone();

        create_link.base_address = entry_hash.clone().into();
        delete_link.base_address = entry_hash.clone().into();
        let create_link_action = Action::CreateLink(create_link.clone());
        let delete_action = Action::Delete(delete.clone());
        let update_action = Action::Update(update.clone());
        let delete_hash = ActionHash::with_data_sync(&delete_action);
        let update_hash = ActionHash::with_data_sync(&update_action);

        let create_link_hash = ActionHash::with_data_sync(&create_link_action);
        delete_link.link_add_address = create_link_hash.clone();
        let delete_link_action = Action::DeleteLink(delete_link.clone());

        let signature = fixt!(Signature);
        let store_entry_op = ChainOpHashed::from_content_sync(ChainOp::StoreEntry(
            signature.clone(),
            NewEntryAction::Create(create.clone()),
            entry.clone(),
        ));

        let wire_create = Judged::valid(SignedAction(create_action, signature).try_into().unwrap());

        let signature = fixt!(Signature);
        let delete_entry_action_op = ChainOpHashed::from_content_sync(
            ChainOp::RegisterDeletedEntryAction(signature.clone(), delete),
        );

        let wire_delete = Judged::valid(SignedAction(delete_action, signature).try_into().unwrap());

        let signature = fixt!(Signature);
        let update_content_op = ChainOpHashed::from_content_sync(ChainOp::RegisterUpdatedContent(
            signature.clone(),
            update,
            update_entry.into(),
        ));
        let wire_update = Judged::valid(SignedAction(update_action, signature).try_into().unwrap());

        let signature = fixt!(Signature);
        let create_link_op = ChainOpHashed::from_content_sync(ChainOp::RegisterAddLink(
            signature.clone(),
            create_link.clone(),
        ));
        let wire_create_link = WireCreateLink::condense(
            create_link_action.clone().try_into().unwrap(),
            signature.clone(),
            ValidationStatus::Valid,
        );
        let wire_create_link_base = WireCreateLink::condense(
            create_link_action.try_into().unwrap(),
            signature.clone(),
            ValidationStatus::Valid,
        );

        let create_link_action = SignedActionHashed::with_presigned(
            ActionHashed::from_content_sync(Action::CreateLink(create_link.clone())),
            signature,
        );

        let signature = fixt!(Signature);
        let delete_link_op = ChainOpHashed::from_content_sync(ChainOp::RegisterRemoveLink(
            signature.clone(),
            delete_link.clone(),
        ));
        let wire_delete_link = WireDeleteLink::condense(
            delete_link_action.try_into().unwrap(),
            signature.clone(),
            ValidationStatus::Valid,
        );
        let delete_link_action = SignedActionHashed::with_presigned(
            ActionHashed::from_content_sync(Action::DeleteLink(delete_link)),
            signature,
        );

        let link_key = WireLinkKey {
            base: create_link.base_address.clone(),
            type_query: LinkTypeFilter::single_dep(0.into()),
            tag: None,
            after: None,
            before: None,
            author: None,
        };
        let link_key_tag = WireLinkKey {
            base: create_link.base_address.clone(),
            type_query: LinkTypeFilter::single_dep(0.into()),
            tag: Some(create_link.tag.clone()),
            after: None,
            before: None,
            author: None,
        };

        let link = Link {
            author: create_link.author,
            base: create_link.base_address.clone(),
            target: create_link.target_address.clone(),
            timestamp: create_link.timestamp,
            zome_index: create_link.zome_index,
            link_type: create_link.link_type,
            tag: create_link.tag,
            create_link_hash,
        };

        let link_query = WireLinkQuery {
            base: create_link.base_address,
            link_type: LinkTypeFilter::single_dep(0.into()),
            tag_prefix: None,
            before: None,
            after: None,
            author: None,
        };

        let entry = EntryData {
            entry,
            entry_type: create.entry_type,
        };

        Self {
            store_entry_op,
            delete_entry_action_op,
            update_content_op,
            hash: entry_hash,
            entry,
            wire_create,
            wire_delete,
            wire_update,
            create_hash,
            delete_hash,
            update_hash,
            create_link_op,
            delete_link_op,
            wire_create_link,
            wire_delete_link,
            link_key,
            link_key_tag,
            links: vec![link],
            create_link_action,
            delete_link_action,
            wire_create_link_base,
            link_query,
        }
    }
}
