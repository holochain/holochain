#![allow(clippy::clippy::redundant_clone)]
use ::fixt::prelude::*;
use holo_hash::*;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::{dht_op::DhtOp, header::NewEntryHeader};
use holochain_zome_types::*;

use super::link::*;
use super::link_details::GetLinkDetailsQuery;
use super::live_entry::*;

pub struct LinkTestData {
    pub create_link_op: DhtOpHashed,
    pub later_create_link_op: DhtOpHashed,
    pub delete_link_op: DhtOpHashed,
    pub link: Link,
    pub later_link: Link,
    pub base_op: DhtOpHashed,
    pub target_op: DhtOpHashed,
    pub base_query: GetLinksQuery,
    pub tag_query: GetLinksQuery,
    pub details_tag_query: GetLinkDetailsQuery,
    pub create_link_header: SignedHeaderHashed,
    pub later_create_link_header: SignedHeaderHashed,
}

pub struct EntryTestData {
    pub store_entry_op: DhtOpHashed,
    pub update_store_entry_op: DhtOpHashed,
    pub delete_entry_header_op: DhtOpHashed,
    pub entry: Entry,
    pub hash: EntryHash,
    pub query: GetLiveEntryQuery,
    pub header: SignedHeaderHashed,
    pub update_header: SignedHeaderHashed,
}

pub struct ElementTestData {
    pub store_element_op: DhtOpHashed,
    pub update_store_element_op: DhtOpHashed,
    pub delete_by_op: DhtOpHashed,
    pub entry: Entry,
    pub header: SignedHeaderHashed,
    pub update_header: SignedHeaderHashed,
    pub create_hash: HeaderHash,
    pub update_hash: HeaderHash,
}

impl LinkTestData {
    pub fn new() -> Self {
        let mut create_link = fixt!(CreateLink);
        let mut later_create_link = create_link.clone();
        let mut delete_link = fixt!(DeleteLink);

        let now = holochain_types::timestamp::now();
        let before = (now - std::time::Duration::from_secs(10)).unwrap();

        create_link.timestamp = before;
        later_create_link.timestamp = now;

        let mut create_base = fixt!(Create);
        let base = Entry::App(fixt!(AppEntryBytes));
        let base_hash = EntryHash::with_data_sync(&base);
        create_base.entry_hash = base_hash.clone();

        let mut create_target = fixt!(Create);
        let target = Entry::App(fixt!(AppEntryBytes));
        let target_hash = EntryHash::with_data_sync(&target);
        create_target.entry_hash = target_hash.clone();

        create_link.base_address = base_hash.clone();
        later_create_link.base_address = base_hash.clone();
        create_link.target_address = target_hash.clone();
        later_create_link.target_address = target_hash.clone();

        let create_link_sig = fixt!(Signature);
        let create_link_op = DhtOp::RegisterAddLink(create_link_sig.clone(), create_link.clone());
        let create_link_header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::CreateLink(create_link.clone())),
            create_link_sig,
        );
        let later_create_link_sig = fixt!(Signature);
        let later_create_link_op =
            DhtOp::RegisterAddLink(later_create_link_sig.clone(), later_create_link.clone());

        let later_create_link_header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::CreateLink(later_create_link.clone())),
            later_create_link_sig,
        );

        let create_link_hash = HeaderHash::with_data_sync(&Header::CreateLink(create_link.clone()));
        let later_create_link_hash =
            HeaderHash::with_data_sync(&Header::CreateLink(later_create_link.clone()));

        delete_link.link_add_address = create_link_hash.clone();
        delete_link.base_address = base_hash.clone();

        let delete_link_op = DhtOp::RegisterRemoveLink(fixt!(Signature), delete_link.clone());

        let base_op = DhtOp::StoreEntry(
            fixt!(Signature),
            NewEntryHeader::Create(create_base.clone()),
            Box::new(base.clone()),
        );

        let target_op = DhtOp::StoreEntry(
            fixt!(Signature),
            NewEntryHeader::Create(create_target.clone()),
            Box::new(target.clone()),
        );

        let link = Link {
            target: target_hash.clone(),
            timestamp: create_link.timestamp,
            tag: create_link.tag.clone(),
            create_link_hash: create_link_hash.clone(),
        };

        let later_link = Link {
            target: target_hash.clone(),
            timestamp: later_create_link.timestamp,
            tag: later_create_link.tag.clone(),
            create_link_hash: later_create_link_hash.clone(),
        };

        let base_query = GetLinksQuery::base(base_hash.clone(), create_link.zome_id);
        let tag_query = GetLinksQuery::tag(
            base_hash.clone(),
            create_link.zome_id,
            create_link.tag.clone(),
        );
        let details_tag_query = GetLinkDetailsQuery::tag(
            base_hash.clone(),
            create_link.zome_id,
            create_link.tag.clone(),
        );

        Self {
            create_link_op: DhtOpHashed::from_content_sync(create_link_op),
            later_create_link_op: DhtOpHashed::from_content_sync(later_create_link_op),
            delete_link_op: DhtOpHashed::from_content_sync(delete_link_op),
            link,
            base_op: DhtOpHashed::from_content_sync(base_op),
            target_op: DhtOpHashed::from_content_sync(target_op),
            base_query,
            tag_query,
            later_link,
            details_tag_query,
            create_link_header,
            later_create_link_header,
        }
    }
}

impl EntryTestData {
    pub fn new() -> Self {
        let mut create = fixt!(Create);
        let mut update = fixt!(Update);
        let mut delete = fixt!(Delete);
        let entry = Entry::App(fixt!(AppEntryBytes));
        let entry_hash = EntryHash::with_data_sync(&entry);
        create.entry_hash = entry_hash.clone();
        update.entry_hash = entry_hash.clone();

        let create_hash = HeaderHash::with_data_sync(&Header::Create(create.clone()));

        delete.deletes_entry_address = entry_hash.clone();
        delete.deletes_address = create_hash.clone();

        let signature = fixt!(Signature);
        let store_entry_op = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryHeader::Create(create.clone()),
            Box::new(entry.clone()),
        ));

        let header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::Create(create.clone())),
            signature.clone(),
        );

        let signature = fixt!(Signature);
        let delete_entry_header_op = DhtOpHashed::from_content_sync(
            DhtOp::RegisterDeletedEntryHeader(signature.clone(), delete.clone()),
        );

        let signature = fixt!(Signature);
        let update_store_entry_op = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryHeader::Update(update.clone()),
            Box::new(entry.clone()),
        ));

        let update_header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::Update(update.clone())),
            signature.clone(),
        );
        let query = GetLiveEntryQuery::new(entry_hash.clone());

        Self {
            store_entry_op,
            header,
            update_store_entry_op,
            update_header,
            entry,
            query,
            delete_entry_header_op,
            hash: entry_hash,
        }
    }
}

impl ElementTestData {
    pub fn new() -> Self {
        let mut create = fixt!(Create);
        let mut update = fixt!(Update);
        let mut delete = fixt!(Delete);
        let entry = fixt!(Entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        create.entry_hash = entry_hash.clone();
        update.entry_hash = entry_hash.clone();

        let create_hash = HeaderHash::with_data_sync(&Header::Create(create.clone()));
        let update_hash = HeaderHash::with_data_sync(&Header::Update(update.clone()));

        delete.deletes_entry_address = entry_hash.clone();
        delete.deletes_address = create_hash.clone();

        let signature = fixt!(Signature);
        let store_element_op = DhtOpHashed::from_content_sync(DhtOp::StoreElement(
            signature.clone(),
            Header::Create(create.clone()),
            Some(Box::new(entry.clone())),
        ));

        let header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::Create(create.clone())),
            signature.clone(),
        );

        let signature = fixt!(Signature);
        let delete_by_op = DhtOpHashed::from_content_sync(DhtOp::RegisterDeletedBy(
            signature.clone(),
            delete.clone(),
        ));

        let signature = fixt!(Signature);
        let update_store_element_op = DhtOpHashed::from_content_sync(DhtOp::StoreElement(
            signature.clone(),
            Header::Update(update.clone()),
            Some(Box::new(entry.clone())),
        ));

        let update_header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::Update(update.clone())),
            signature.clone(),
        );

        Self {
            store_element_op,
            header,
            update_store_element_op,
            update_header,
            entry,
            delete_by_op,
            create_hash,
            update_hash,
        }
    }
}
