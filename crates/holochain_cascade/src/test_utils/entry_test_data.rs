use holo_hash::ActionHash;
use holo_hash::EntryHash;
use holochain_serialized_bytes::UnsafeBytes;
use holochain_types::action::NewEntryAction;
use holochain_types::action::WireDelete;
use holochain_types::action::WireNewEntryAction;
use holochain_types::action::WireUpdateRelationship;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::link::WireCreateLink;
use holochain_types::link::WireDeleteLink;
use holochain_types::link::WireLinkKey;
use holochain_types::prelude::EntryData;
use holochain_zome_types::fixt::*;
use holochain_zome_types::Action;
use holochain_zome_types::ActionHashed;
use holochain_zome_types::AppEntryBytes;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryType;
use holochain_zome_types::EntryVisibility;
use holochain_zome_types::Judged;
use holochain_zome_types::Link;
use holochain_zome_types::LinkTypeRange;
use holochain_zome_types::SerializedBytes;
use holochain_zome_types::SignedAction;
use holochain_zome_types::SignedActionHashed;
use holochain_zome_types::ValidationStatus;
use std::convert::TryInto;

use ::fixt::prelude::*;
#[derive(Debug)]
pub struct EntryTestData {
    pub store_entry_op: DhtOpHashed,
    pub wire_create: Judged<WireNewEntryAction>,
    pub create_hash: ActionHash,
    pub delete_entry_action_op: DhtOpHashed,
    pub wire_delete: Judged<WireDelete>,
    pub delete_hash: ActionHash,
    pub update_content_op: DhtOpHashed,
    pub wire_update: Judged<WireUpdateRelationship>,
    pub update_hash: ActionHash,
    pub hash: EntryHash,
    pub entry: EntryData,
    // Links
    pub create_link_op: DhtOpHashed,
    pub delete_link_op: DhtOpHashed,
    pub wire_create_link: WireCreateLink,
    pub wire_create_link_base: WireCreateLink,
    pub wire_delete_link: WireDeleteLink,
    pub create_link_action: SignedActionHashed,
    pub delete_link_action: SignedActionHashed,
    pub link_key: WireLinkKey,
    pub link_key_tag: WireLinkKey,
    pub links: Vec<Link>,
}

impl EntryTestData {
    pub fn create() -> Self {
        let mut create = fixt!(Create);
        let mut update = fixt!(Update);
        let mut delete = fixt!(Delete);

        let mut create_link = fixt!(CreateLink);
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
            AppEntryTypeFixturator::new(EntryVisibility::Public).map(EntryType::App);

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
        let store_entry_op = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryAction::Create(create.clone()),
            Box::new(entry.clone()),
        ));

        let wire_create = Judged::valid(SignedAction(create_action, signature).try_into().unwrap());

        let signature = fixt!(Signature);
        let delete_entry_action_op = DhtOpHashed::from_content_sync(
            DhtOp::RegisterDeletedEntryAction(signature.clone(), delete),
        );

        let wire_delete = Judged::valid(SignedAction(delete_action, signature).try_into().unwrap());

        let signature = fixt!(Signature);
        let update_content_op = DhtOpHashed::from_content_sync(DhtOp::RegisterUpdatedContent(
            signature.clone(),
            update,
            Some(Box::new(update_entry)),
        ));
        let wire_update = Judged::valid(SignedAction(update_action, signature).try_into().unwrap());

        let signature = fixt!(Signature);
        let create_link_op = DhtOpHashed::from_content_sync(DhtOp::RegisterAddLink(
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
        let delete_link_op = DhtOpHashed::from_content_sync(DhtOp::RegisterRemoveLink(
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
            type_query: Some(LinkTypeRange::Full.into()),
            tag: None,
        };
        let link_key_tag = WireLinkKey {
            base: create_link.base_address.clone(),
            type_query: Some(LinkTypeRange::Full.into()),
            tag: Some(create_link.tag.clone()),
        };

        let link = Link {
            target: create_link.target_address.clone(),
            timestamp: create_link.timestamp,
            tag: create_link.tag,
            create_link_hash,
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
        }
    }
}
