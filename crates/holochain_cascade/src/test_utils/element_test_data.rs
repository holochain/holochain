use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::header::WireDelete;
use holochain_types::header::WireUpdateRelationship;
use holochain_zome_types::fixt::*;
use holochain_zome_types::Create;
use holochain_zome_types::Element;
use holochain_zome_types::Entry;
use holochain_zome_types::Header;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::Judged;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::SignedHeaderHashed;
use holochain_zome_types::Update;
use std::convert::TryInto;

use ::fixt::prelude::*;
#[derive(Debug)]
pub struct ElementTestData {
    pub store_element_op: DhtOpHashed,
    pub wire_create: Judged<SignedHeader>,
    pub create_hash: HeaderHash,
    pub deleted_by_op: DhtOpHashed,
    pub wire_delete: Judged<WireDelete>,
    pub delete_hash: HeaderHash,
    pub update_element_op: DhtOpHashed,
    pub wire_update: Judged<WireUpdateRelationship>,
    pub update_hash: HeaderHash,
    pub hash: EntryHash,
    pub entry: Entry,
    pub any_store_element_op: DhtOpHashed,
    pub any_header: Judged<SignedHeader>,
    pub any_header_hash: HeaderHash,
    pub any_entry: Option<Entry>,
    pub any_entry_hash: Option<EntryHash>,
    pub any_element: Element,
}

impl ElementTestData {
    pub fn create() -> Self {
        let mut create = fixt!(Create);
        let mut update = fixt!(Update);
        let mut delete = fixt!(Delete);
        let mut any_header = fixt!(Header);
        let entry = fixt!(AppEntryBytes);
        let entry = Entry::App(entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        let update_entry = fixt!(AppEntryBytes);
        let update_entry = Entry::App(update_entry);
        let update_entry_hash = EntryHash::with_data_sync(&update_entry);

        create.entry_hash = entry_hash.clone();
        update.entry_hash = update_entry_hash;

        let create_header = Header::Create(create);
        let create_hash = HeaderHash::with_data_sync(&create_header);

        delete.deletes_address = create_hash.clone();
        delete.deletes_entry_address = entry_hash.clone();

        update.original_entry_address = entry_hash.clone();
        update.original_header_address = create_hash.clone();

        let delete_header = Header::Delete(delete.clone());
        let update_header = Header::Update(update.clone());
        let delete_hash = HeaderHash::with_data_sync(&delete_header);
        let update_hash = HeaderHash::with_data_sync(&update_header);

        let signature = fixt!(Signature);
        let store_element_op = DhtOpHashed::from_content_sync(DhtOp::StoreElement(
            signature.clone(),
            create_header.clone(),
            Some(Box::new(entry.clone())),
        ));

        let wire_create = Judged::valid(SignedHeader(create_header, signature));

        let signature = fixt!(Signature);
        let deleted_by_op =
            DhtOpHashed::from_content_sync(DhtOp::RegisterDeletedBy(signature.clone(), delete));

        let wire_delete = Judged::valid(SignedHeader(delete_header, signature).try_into().unwrap());

        let signature = fixt!(Signature);
        let update_element_op = DhtOpHashed::from_content_sync(DhtOp::RegisterUpdatedElement(
            signature.clone(),
            update,
            Some(Box::new(update_entry)),
        ));
        let wire_update = Judged::valid(SignedHeader(update_header, signature).try_into().unwrap());

        let mut any_entry = None;
        let mut any_entry_hash = None;
        if any_header.entry_hash().is_some() {
            match &mut any_header {
                Header::Create(Create { entry_hash: eh, .. })
                | Header::Update(Update { entry_hash: eh, .. }) => {
                    let entry = fixt!(AppEntryBytes);
                    let entry = Entry::App(entry);
                    *eh = EntryHash::with_data_sync(&entry);
                    any_entry_hash = Some(eh.clone());
                    any_entry = Some(Box::new(entry));
                }
                _ => unreachable!(),
            }
        }

        let any_header_hash = HeaderHash::with_data_sync(&any_header);

        let signature = fixt!(Signature);
        let any_store_element_op = DhtOpHashed::from_content_sync(DhtOp::StoreElement(
            signature.clone(),
            any_header.clone(),
            any_entry.clone(),
        ));

        let any_element = Element::new(
            SignedHeaderHashed::with_presigned(
                HeaderHashed::from_content_sync(any_header.clone()),
                signature.clone(),
            ),
            any_entry.clone().map(|i| *i),
        );

        let any_header = Judged::valid(SignedHeader(any_header, signature));

        Self {
            store_element_op,
            deleted_by_op,
            update_element_op,
            hash: entry_hash,
            entry,
            wire_create,
            wire_delete,
            wire_update,
            create_hash,
            delete_hash,
            update_hash,
            any_store_element_op,
            any_header,
            any_header_hash,
            any_entry: any_entry.map(|e| *e),
            any_entry_hash,
            any_element,
        }
    }
}
