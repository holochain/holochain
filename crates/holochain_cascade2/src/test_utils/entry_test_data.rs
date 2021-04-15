use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::header::NewEntryHeader;
use holochain_zome_types::fixt::*;
use holochain_zome_types::Entry;
use holochain_zome_types::Header;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::Link;
use holochain_zome_types::SignedHeaderHashed;
use holochain_zome_types::ValidationStatus;

use crate::authority::WireDhtOp;
use crate::authority::WireLinkKey;
use ::fixt::prelude::*;
#[derive(Debug)]
pub struct EntryTestData {
    pub store_entry_op: DhtOpHashed,
    pub wire_create: WireDhtOp,
    pub create_hash: HeaderHash,
    pub delete_entry_header_op: DhtOpHashed,
    pub wire_delete: WireDhtOp,
    pub delete_hash: HeaderHash,
    pub update_content_op: DhtOpHashed,
    pub wire_update: WireDhtOp,
    pub update_hash: HeaderHash,
    pub hash: EntryHash,
    pub entry: Entry,
    // Links
    pub create_link_op: DhtOpHashed,
    pub delete_link_op: DhtOpHashed,
    pub wire_create_link: WireDhtOp,
    pub wire_delete_link: WireDhtOp,
    pub create_link_header: SignedHeaderHashed,
    pub delete_link_header: SignedHeaderHashed,
    pub link_key: WireLinkKey,
    pub link_key_tag: WireLinkKey,
    pub links: Vec<Link>,
}

impl EntryTestData {
    pub fn new() -> Self {
        let mut create = fixt!(Create);
        let mut update = fixt!(Update);
        let mut delete = fixt!(Delete);

        let mut create_link = fixt!(CreateLink);
        let mut delete_link = fixt!(DeleteLink);

        let entry = fixt!(AppEntryBytes);
        let entry = Entry::App(entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        let update_entry = fixt!(AppEntryBytes);
        let update_entry = Entry::App(update_entry);
        let update_entry_hash = EntryHash::with_data_sync(&update_entry);

        create.entry_hash = entry_hash.clone();
        update.entry_hash = update_entry_hash.clone();

        let create_header = Header::Create(create.clone());
        let create_hash = HeaderHash::with_data_sync(&create_header);

        delete.deletes_entry_address = entry_hash.clone();
        delete.deletes_address = create_hash.clone();

        update.original_entry_address = entry_hash.clone();
        update.original_header_address = create_hash.clone();

        create_link.base_address = entry_hash.clone();
        delete_link.base_address = entry_hash.clone();
        let create_link_header = Header::CreateLink(create_link.clone());
        let delete_header = Header::Delete(delete.clone());
        let update_header = Header::Update(update.clone());
        let delete_hash = HeaderHash::with_data_sync(&delete_header);
        let update_hash = HeaderHash::with_data_sync(&update_header);

        let create_link_hash = HeaderHash::with_data_sync(&create_link_header);
        delete_link.link_add_address = create_link_hash.clone();
        let delete_link_header = Header::DeleteLink(delete_link.clone());

        let signature = fixt!(Signature);
        let store_entry_op = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryHeader::Create(create.clone()),
            Box::new(entry.clone()),
        ));

        let wire_create = WireDhtOp {
            op_type: store_entry_op.as_content().get_type(),
            header: create_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let signature = fixt!(Signature);
        let delete_entry_header_op = DhtOpHashed::from_content_sync(
            DhtOp::RegisterDeletedEntryHeader(signature.clone(), delete.clone()),
        );

        let wire_delete = WireDhtOp {
            op_type: delete_entry_header_op.as_content().get_type(),
            header: delete_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let signature = fixt!(Signature);
        let update_content_op = DhtOpHashed::from_content_sync(DhtOp::RegisterUpdatedContent(
            signature.clone(),
            update.clone(),
            Some(Box::new(update_entry)),
        ));
        let wire_update = WireDhtOp {
            op_type: update_content_op.as_content().get_type(),
            header: update_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };

        let signature = fixt!(Signature);
        let create_link_op = DhtOpHashed::from_content_sync(DhtOp::RegisterAddLink(
            signature.clone(),
            create_link.clone(),
        ));
        let wire_create_link = WireDhtOp {
            op_type: create_link_op.as_content().get_type(),
            header: create_link_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };
        let create_link_header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::CreateLink(create_link.clone())),
            signature,
        );

        let signature = fixt!(Signature);
        let delete_link_op = DhtOpHashed::from_content_sync(DhtOp::RegisterRemoveLink(
            signature.clone(),
            delete_link.clone(),
        ));
        let wire_delete_link = WireDhtOp {
            op_type: delete_link_op.as_content().get_type(),
            header: delete_link_header.clone(),
            signature: signature.clone(),
            validation_status: Some(ValidationStatus::Valid),
        };
        let delete_link_header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::DeleteLink(delete_link.clone())),
            signature,
        );

        let link_key = WireLinkKey {
            base: create_link.base_address.clone(),
            zome_id: create_link.zome_id,
            tag: None,
        };
        let link_key_tag = WireLinkKey {
            base: create_link.base_address.clone(),
            zome_id: create_link.zome_id,
            tag: Some(create_link.tag.clone()),
        };

        let link = Link {
            target: create_link.target_address.clone(),
            timestamp: create_link.timestamp.clone(),
            tag: create_link.tag.clone(),
            create_link_hash: create_link_hash.clone(),
        };

        Self {
            store_entry_op,
            delete_entry_header_op,
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
            create_link_header,
            delete_link_header,
        }
    }
}
