use super::*;
use ::fixt::prelude::*;
use ghost_actor::dependencies::observability;
use holochain_sqlite::db::WriteManager;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::insert::insert_op;
use holochain_state::prelude::test_cell_env;

struct EntryTestData {
    store_entry_op: DhtOpHashed,
    delete_entry_header_op: DhtOpHashed,
    hash: EntryHash,
}

impl EntryTestData {
    fn new() -> Self {
        let mut create = fixt!(Create);
        let mut delete = fixt!(Delete);
        let entry = fixt!(Entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        create.entry_hash = entry_hash.clone();

        let create_hash = HeaderHash::with_data_sync(&Header::Create(create.clone()));

        delete.deletes_entry_address = entry_hash.clone();
        delete.deletes_address = create_hash.clone();

        let signature = fixt!(Signature);
        let store_entry_op = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryHeader::Create(create.clone()),
            Box::new(entry.clone()),
        ));

        let signature = fixt!(Signature);
        let delete_entry_header_op = DhtOpHashed::from_content_sync(
            DhtOp::RegisterDeletedEntryHeader(signature.clone(), delete.clone()),
        );

        Self {
            store_entry_op,
            delete_entry_header_op,
            hash: entry_hash,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_entry() {
    observability::test_run().ok();
    let env = test_cell_env();

    let td = EntryTestData::new();

    fill_db(&env.env(), td.store_entry_op.clone());

    // TODO: These are probably irrelevant now
    let options = holochain_p2p::event::GetOptions {
        follow_redirects: false,
        all_live_headers_with_metadata: true,
    };

    let result = handle_get_entry(env.env().into(), td.hash.clone(), options.clone()).unwrap();
    let expected = WireEntryOps {
        creates: vec![td.store_entry_op.clone().into_content()],
        deletes: vec![],
        updates: vec![],
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.delete_entry_header_op.clone());

    let result = handle_get_entry(env.env().into(), td.hash.clone(), options.clone()).unwrap();
    let expected = WireEntryOps {
        creates: vec![td.store_entry_op.clone().into_content()],
        deletes: vec![td.delete_entry_header_op.clone().into_content()],
        updates: vec![],
    };
    assert_eq!(result, expected);
}

fn fill_db(env: &EnvWrite, op: DhtOpHashed) {
    env.conn()
        .unwrap()
        .with_commit(|txn| {
            insert_op(txn, op);
            DatabaseResult::Ok(())
        })
        .unwrap();
}
