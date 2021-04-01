use super::*;
use ::fixt::prelude::*;
use get_entry_query::WireDhtOp;
use ghost_actor::dependencies::observability;
use holochain_sqlite::db::WriteManager;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::insert::insert_op;
use holochain_state::prelude::test_cell_env;

struct EntryTestData {
    store_entry_op: DhtOpHashed,
    wire_create: WireDhtOp,
    delete_entry_header_op: DhtOpHashed,
    wire_delete: WireDhtOp,
    hash: EntryHash,
    entry: Entry,
}

impl EntryTestData {
    fn new() -> Self {
        let mut create = fixt!(Create);
        let mut delete = fixt!(Delete);
        let entry = fixt!(Entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        create.entry_hash = entry_hash.clone();

        let create_header = Header::Create(create.clone());
        let create_hash = HeaderHash::with_data_sync(&create_header);

        delete.deletes_entry_address = entry_hash.clone();
        delete.deletes_address = create_hash.clone();

        let delete_header = Header::Delete(delete.clone());

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
        };

        let signature = fixt!(Signature);
        let delete_entry_header_op = DhtOpHashed::from_content_sync(
            DhtOp::RegisterDeletedEntryHeader(signature.clone(), delete.clone()),
        );

        let wire_delete = WireDhtOp {
            op_type: delete_entry_header_op.as_content().get_type(),
            header: delete_header.clone(),
            signature: signature.clone(),
        };

        Self {
            store_entry_op,
            delete_entry_header_op,
            hash: entry_hash,
            entry,
            wire_create,
            wire_delete,
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
        creates: vec![td.wire_create.clone()],
        deletes: vec![],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.delete_entry_header_op.clone());

    let result = handle_get_entry(env.env().into(), td.hash.clone(), options.clone()).unwrap();
    let expected = WireEntryOps {
        creates: vec![td.wire_create.clone()],
        deletes: vec![td.wire_delete.clone()],
        updates: vec![],
        entry: Some(td.entry.clone()),
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
