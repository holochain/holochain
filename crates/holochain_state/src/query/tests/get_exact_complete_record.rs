use super::*;
use crate::prelude::mutations_helpers::insert_valid_integrated_op;
use holo_hash::AnyDhtHash;
use holochain_sqlite::rusqlite::TransactionBehavior;
use holochain_sqlite::{rusqlite::Connection, schema::SCHEMA_CELL};
use holochain_types::action::NewEntryAction;

#[tokio::test(flavor = "multi_thread")]
async fn returns_record_when_entry_present() {
    holochain_trace::test_run();
    let mut test_case = TestCase::new();
    let mut txn = test_case.transaction();

    let op_types_with_entry = [
        ChainOpType::StoreRecord,
        ChainOpType::StoreEntry,
        ChainOpType::RegisterUpdatedContent,
        ChainOpType::RegisterUpdatedRecord,
    ];

    for op_type in op_types_with_entry {
        let op = create_test_chain_op(op_type);
        let action_hash = op.action().to_hash();

        // Insert the operation into the database
        insert_valid_integrated_op(&mut txn, &op.downcast()).unwrap();

        let expected_record = Record::new(
            SignedActionHashed::from_content_sync(op.signed_action()),
            op.entry().into_option().cloned(),
        );
        let actual_record = CascadeTxnWrapper::from(&txn)
            .get_complete_public_record(&action_hash)
            .unwrap();
        assert!(actual_record.is_some(), "Failed for op type {op_type}");
        let actual_record = actual_record.unwrap();
        assert_eq!(
            actual_record, expected_record,
            "Failed for op type {op_type}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn returns_none_when_no_entry_present() {
    holochain_trace::test_run();
    let mut test_case = TestCase::new();
    let mut txn = test_case.transaction();

    let op_types_with_entry = [
        ChainOpType::RegisterAgentActivity,
        ChainOpType::RegisterAddLink,
        ChainOpType::RegisterRemoveLink,
        ChainOpType::RegisterDeletedBy,
        ChainOpType::RegisterDeletedEntryAction,
    ];

    for op_type in op_types_with_entry {
        let op = create_test_chain_op(op_type);

        let action_hash = op.action().to_hash();

        // Insert the operation into the database
        insert_valid_integrated_op(&mut txn, &op.downcast()).unwrap();

        let result = CascadeTxnWrapper::from(&txn)
            .get_public_record(&AnyDhtHash::from(action_hash.clone()))
            .unwrap();
        assert!(result.is_none(), "Failed for op type {op_type}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn returns_none_when_private_entry_present() {
    holochain_trace::test_run();
    let mut test_case = TestCase::new();
    let mut txn = test_case.transaction();

    let mut create = fixt!(Create);
    let entry = Entry::App(fixt!(AppEntryBytes));
    create.entry_type = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Private, // <- private entry
    ));
    create.entry_hash = entry.to_hash();
    let action = NewEntryAction::Create(create);
    let op = ChainOpHashed::from_content_sync(ChainOp::StoreEntry(fixt!(Signature), action, entry));

    let action_hash = op.action().to_hash();

    // Insert the operation into the database
    insert_valid_integrated_op(&mut txn, &op.downcast()).unwrap();

    let result = CascadeTxnWrapper::from(&txn)
        .get_public_record(&AnyDhtHash::from(action_hash.clone()))
        .unwrap();
    assert!(result.is_none(), "Record with private entry returned");
}

struct TestCase {
    conn: Connection,
}

impl TestCase {
    fn new() -> Self {
        let mut conn = Connection::open_in_memory().unwrap();
        SCHEMA_CELL.initialize(&mut conn, None).unwrap();
        Self { conn }
    }

    fn transaction(&mut self) -> Transaction<'_> {
        self.conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .unwrap()
    }
}

/// Create chain ops for testing with correctly hooked up entries.
pub fn create_test_chain_op(op_type: ChainOpType) -> ChainOpHashed {
    let chain_op = match op_type {
        ChainOpType::StoreRecord => {
            let mut create = fixt!(Create);
            let entry = Entry::App(fixt!(AppEntryBytes));
            create.entry_type = EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            ));
            create.entry_hash = entry.to_hash();
            let action = Action::Create(create);
            ChainOp::StoreRecord(fixt!(Signature), action, RecordEntry::Present(entry))
        }
        ChainOpType::StoreEntry => {
            let mut create = fixt!(Create);
            let entry = Entry::App(fixt!(AppEntryBytes));
            create.entry_type = EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            ));
            create.entry_hash = entry.to_hash();
            let action = NewEntryAction::Create(create);
            ChainOp::StoreEntry(fixt!(Signature), action, entry)
        }
        ChainOpType::RegisterAgentActivity => {
            ChainOp::RegisterAgentActivity(fixt!(Signature), fixt!(Action))
        }
        ChainOpType::RegisterUpdatedContent => {
            let mut update = fixt!(Update);
            let entry = Entry::App(fixt!(AppEntryBytes));
            update.entry_type = EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            ));
            update.entry_hash = entry.to_hash();
            ChainOp::RegisterUpdatedContent(fixt!(Signature), update, RecordEntry::Present(entry))
        }
        ChainOpType::RegisterUpdatedRecord => {
            let mut update = fixt!(Update);
            let entry = Entry::App(fixt!(AppEntryBytes));
            update.entry_type = EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            ));
            update.entry_hash = entry.to_hash();
            ChainOp::RegisterUpdatedRecord(fixt!(Signature), update, RecordEntry::Present(entry))
        }
        ChainOpType::RegisterDeletedBy => {
            ChainOp::RegisterDeletedBy(fixt!(Signature), fixt!(Delete))
        }
        ChainOpType::RegisterDeletedEntryAction => {
            ChainOp::RegisterDeletedEntryAction(fixt!(Signature), fixt!(Delete))
        }
        ChainOpType::RegisterAddLink => {
            ChainOp::RegisterAddLink(fixt!(Signature), fixt!(CreateLink))
        }
        ChainOpType::RegisterRemoveLink => {
            ChainOp::RegisterRemoveLink(fixt!(Signature), fixt!(DeleteLink))
        }
    };
    ChainOpHashed::from_content_sync(chain_op)
}
