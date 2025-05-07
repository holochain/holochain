use ::fixt::fixt;
use holo_hash::HashableContentExtSync;
use holochain_cascade::CascadeImpl;
use holochain_state::prelude::{
    insert_op_authored, insert_op_cache, insert_op_dht, set_validation_status, set_when_integrated,
    test_authored_db, test_cache_db, test_dht_db,
};
use holochain_types::{
    dht_op::{ChainOp, DhtOp},
    fixt::{
        CreateFixturator, CreateLinkFixturator, DeleteFixturator, DeleteLinkFixturator,
        SignatureFixturator, UpdateFixturator,
    },
    prelude::{
        AppEntryDef, EntryType, EntryVisibility, GetOptions, RecordEntry, Timestamp,
        ValidationStatus,
    },
};
use holochain_zome_types::{Action, Entry};

#[tokio::test(flavor = "multi_thread")]
async fn get_action_from_authored() {
    let authored = test_authored_db();
    let cascade = CascadeImpl::empty().with_authored(authored.clone().into());

    let create_action = Action::Create(fixt!(Create));
    let create_op = DhtOp::ChainOp(Box::new(ChainOp::StoreRecord(
        fixt!(Signature),
        create_action.clone(),
        RecordEntry::NA,
    )))
    .into_hashed();
    let op_hash = create_op.hash.clone();

    authored.test_write(move |txn| {
        insert_op_authored(txn, &create_op).unwrap();
    });
    // Get should not return the op while it's not valid and integrated.
    let maybe_record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap();
    assert!(maybe_record.is_none());

    // Set op to valid.
    let op_hash2 = op_hash.clone();
    authored.test_write(move |txn| {
        set_validation_status(txn, &op_hash2, ValidationStatus::Valid).unwrap();
    });
    // Get should not return the op while it's not integrated.
    let maybe_record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap();
    assert!(maybe_record.is_none());

    // Set op to integrated, even though this is the authored database. Queries are applied
    // equally to all 3 databases, so this field must exist and be set for them to return rows.
    authored.test_write(move |txn| {
        set_when_integrated(txn, &op_hash, Timestamp::now()).unwrap();
    });
    // Get should return the record from the op.
    let record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*record.action(), create_action);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_action_from_dht() {
    let dht = test_dht_db();
    let cascade = CascadeImpl::empty().with_dht(dht.clone().into());

    let create_action = Action::Create(fixt!(Create));
    let create_op = DhtOp::ChainOp(Box::new(ChainOp::StoreRecord(
        fixt!(Signature),
        create_action.clone(),
        RecordEntry::NA,
    )))
    .into_hashed();
    let op_hash = create_op.hash.clone();

    dht.test_write(move |txn| {
        insert_op_dht(txn, &create_op, 0, None).unwrap();
    });
    // Get should not return the op while it's not valid and integrated.
    let maybe_record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap();
    assert!(maybe_record.is_none());

    // Set op to valid.
    let op_hash2 = op_hash.clone();
    dht.test_write(move |txn| {
        set_validation_status(txn, &op_hash2, ValidationStatus::Valid).unwrap();
    });
    // Get should not return the op while it's not integrated.
    let maybe_record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap();
    assert!(maybe_record.is_none());

    // Set op to integrated.
    dht.test_write(move |txn| {
        set_when_integrated(txn, &op_hash, Timestamp::now()).unwrap();
    });
    // Get should return the op.
    let maybe_record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap();
    assert!(maybe_record.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn get_action_from_cache() {
    let cache = test_cache_db();
    let cascade = CascadeImpl::empty().with_cache(cache.clone());

    let create_action = Action::Create(fixt!(Create));
    let create_op = DhtOp::ChainOp(Box::new(ChainOp::StoreRecord(
        fixt!(Signature),
        create_action.clone(),
        RecordEntry::NA,
    )))
    .into_hashed();
    let op_hash = create_op.hash.clone();

    cache.test_write(move |txn| {
        insert_op_cache(txn, &create_op).unwrap();
    });
    // Get should not return the op while it's not valid and integrated.
    let maybe_record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap();
    assert!(maybe_record.is_none());

    // Set op to valid.
    let op_hash2 = op_hash.clone();
    cache.test_write(move |txn| {
        set_validation_status(txn, &op_hash2, ValidationStatus::Valid).unwrap();
    });
    // Get should not return the op while it's not integrated.
    let maybe_record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap();
    assert!(maybe_record.is_none());

    // Set op to integrated.
    cache.test_write(move |txn| {
        set_when_integrated(txn, &op_hash, Timestamp::now()).unwrap();
    });
    // Get should return the op.
    let maybe_record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap();
    assert!(maybe_record.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn get_updated_then_deleted_action() {
    let dht = test_dht_db();
    let cascade = CascadeImpl::empty().with_dht(dht.clone().into());

    // Write a create op to the dht db.
    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let create_action = Action::Create(create);
    let create_op = DhtOp::ChainOp(Box::new(ChainOp::StoreRecord(
        fixt!(Signature),
        create_action.clone(),
        RecordEntry::NotStored,
    )))
    .into_hashed();

    let create_op2 = create_op.clone();
    dht.test_write(move |txn| {
        insert_op_dht(txn, &create_op2, 0, None).unwrap();
        set_validation_status(txn, &create_op2.hash, ValidationStatus::Valid).unwrap();
        set_when_integrated(txn, &create_op2.hash, Timestamp::now()).unwrap();
    });

    // Write an update to the dht db. This won't affect the result of the function.
    // This step is included here because updates are included in the query.
    let mut update = fixt!(Update);
    update.original_action_address = create_action.to_hash();
    let update_op = DhtOp::ChainOp(Box::new(ChainOp::RegisterUpdatedRecord(
        fixt!(Signature),
        update,
        RecordEntry::NotStored,
    )))
    .into_hashed();
    dht.test_write(move |txn| {
        insert_op_dht(txn, &update_op, 0, None).unwrap();
        set_validation_status(txn, &update_op.hash, ValidationStatus::Valid).unwrap();
        set_when_integrated(txn, &update_op.hash, Timestamp::now()).unwrap();
    });

    // Get should return the op.
    let record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*record.action(), create_action);
    assert_eq!(
        record.entry().as_ref(),
        create_op.as_chain_op().unwrap().entry()
    );

    // Write a delete to the dht db.
    let mut delete = fixt!(Delete);
    delete.deletes_address = create_action.to_hash();
    let delete_op = DhtOp::ChainOp(Box::new(ChainOp::RegisterDeletedBy(
        fixt!(Signature),
        delete,
    )))
    .into_hashed();
    dht.test_write(move |txn| {
        insert_op_dht(txn, &delete_op, 0, None).unwrap();
        set_validation_status(txn, &delete_op.hash, ValidationStatus::Valid).unwrap();
        set_when_integrated(txn, &delete_op.hash, Timestamp::now()).unwrap();
    });

    // Get should not return the op any more.
    let maybe_record = cascade
        .dht_get(create_action.to_hash().into(), GetOptions::local())
        .await
        .unwrap();
    assert!(maybe_record.is_none());
}

#[test]
fn re() {
    let r = RecordEntry::new(None, None::<Entry>);
    println!("r {r:?}");
    let r = RecordEntry::new(Some(&EntryVisibility::Private), None::<Entry>);
    println!("r {r:?}");
}

// Correctness tests for zero arc nodes.
mod zero_arc {
    use super::*;

    // When deleting a link, the create link action is looked up.
    // This is a special case, because `get_links` fetches ops of type
    // `RegisterAddLink`, but deleting a link fetches the op of type
    // `StoreRecord`.
    #[tokio::test(flavor = "multi_thread")]
    async fn delete_link() {
        let cache = test_cache_db();
        let cascade = CascadeImpl::empty().with_cache(cache.clone());

        let create_link = fixt!(CreateLink);
        let mut delete_link = fixt!(DeleteLink);
        delete_link.link_add_address = create_link.to_hash();
        // Add the `RegisterAddLink` op to the cache, which comes in with `get_links`.
        let create_link_op = DhtOp::ChainOp(Box::new(ChainOp::RegisterAddLink(
            fixt!(Signature),
            create_link.clone(),
        )))
        .into_hashed();
        cache.test_write(move |txn| {
            insert_op_cache(txn, &create_link_op).unwrap();
            set_validation_status(txn, &create_link_op.hash, ValidationStatus::Valid).unwrap();
            set_when_integrated(txn, &create_link_op.hash, Timestamp::now()).unwrap();
        });

        // Get should not return an op, because it's of the wrong type.
        let maybe_create_link_record = cascade
            .dht_get(
                delete_link.link_add_address.clone().into(),
                GetOptions::local(),
            )
            .await
            .unwrap();
        assert!(maybe_create_link_record.is_none());

        // Add the `StoreRecord` op as well.
        let create_link_op = DhtOp::ChainOp(Box::new(ChainOp::StoreRecord(
            fixt!(Signature),
            Action::CreateLink(create_link.clone()),
            RecordEntry::NA,
        )))
        .into_hashed();
        cache.test_write(move |txn| {
            insert_op_cache(txn, &create_link_op).unwrap();
            set_validation_status(txn, &create_link_op.hash, ValidationStatus::Valid).unwrap();
            set_when_integrated(txn, &create_link_op.hash, Timestamp::now()).unwrap();
        });

        // Get should return the op now.
        let maybe_create_link_record = cascade
            .dht_get(delete_link.link_add_address.into(), GetOptions::local())
            .await
            .unwrap();
        assert!(maybe_create_link_record.is_some());
    }
}
