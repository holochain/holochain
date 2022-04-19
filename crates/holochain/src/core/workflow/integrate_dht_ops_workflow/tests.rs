#![cfg(test)]
#![cfg(feature = "test_utils")]

use super::*;

use crate::core::queue_consumer::TriggerSender;
use crate::here;
use crate::test_utils::test_network;
use ::fixt::prelude::*;
use holochain_sqlite::db::WriteManager;
use holochain_state::query::link::GetLinksQuery;
use holochain_state::workspace::WorkspaceError;
use holochain_zome_types::Entry;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::ValidationStatus;
use observability;

#[derive(Clone)]
struct TestData {
    signature: Signature,
    original_entry: Entry,
    new_entry: Entry,
    entry_update_header: Update,
    entry_update_entry: Update,
    original_header_hash: HeaderHash,
    original_entry_hash: EntryHash,
    original_header: NewEntryHeader,
    entry_delete: Delete,
    link_add: CreateLink,
    link_remove: DeleteLink,
}

impl TestData {
    async fn new() -> Self {
        // original entry
        let original_entry = EntryFixturator::new(AppEntry).next().unwrap();
        // New entry
        let new_entry = EntryFixturator::new(AppEntry).next().unwrap();
        Self::new_inner(original_entry, new_entry)
    }

    #[instrument()]
    fn new_inner(original_entry: Entry, new_entry: Entry) -> Self {
        // original entry
        let original_entry_hash =
            EntryHashed::from_content_sync(original_entry.clone()).into_hash();

        // New entry
        let new_entry_hash = EntryHashed::from_content_sync(new_entry.clone()).into_hash();

        // Original entry and header for updates
        let mut original_header = fixt!(NewEntryHeader, PublicCurve);
        debug!(?original_header);

        match &mut original_header {
            NewEntryHeader::Create(c) => c.entry_hash = original_entry_hash.clone(),
            NewEntryHeader::Update(u) => u.entry_hash = original_entry_hash.clone(),
        }

        let original_header_hash =
            HeaderHashed::from_content_sync(original_header.clone().into()).into_hash();

        // Header for the new entry
        let mut new_entry_header = fixt!(NewEntryHeader, PublicCurve);

        // Update to new entry
        match &mut new_entry_header {
            NewEntryHeader::Create(c) => c.entry_hash = new_entry_hash.clone(),
            NewEntryHeader::Update(u) => u.entry_hash = new_entry_hash.clone(),
        }

        // Entry update for header
        let mut entry_update_header = fixt!(Update, PublicCurve);
        entry_update_header.entry_hash = new_entry_hash.clone();
        entry_update_header.original_header_address = original_header_hash.clone();

        // Entry update for entry
        let mut entry_update_entry = fixt!(Update, PublicCurve);
        entry_update_entry.entry_hash = new_entry_hash.clone();
        entry_update_entry.original_entry_address = original_entry_hash.clone();
        entry_update_entry.original_header_address = original_header_hash.clone();

        // Entry delete
        let mut entry_delete = fixt!(Delete);
        entry_delete.deletes_address = original_header_hash.clone();

        // Link add
        let mut link_add = fixt!(CreateLink);
        link_add.base_address = original_entry_hash.clone().into();
        link_add.target_address = new_entry_hash.clone().into();
        link_add.zome_id = fixt!(ZomeId);
        link_add.tag = fixt!(LinkTag);

        let link_add_hash = HeaderHashed::from_content_sync(link_add.clone().into()).into_hash();

        // Link remove
        let mut link_remove = fixt!(DeleteLink);
        link_remove.base_address = original_entry_hash.clone().into();
        link_remove.link_add_address = link_add_hash.clone();

        // Any Header
        let mut any_header = fixt!(Header, PublicCurve);
        match &mut any_header {
            Header::Create(ec) => {
                ec.entry_hash = original_entry_hash.clone();
            }
            Header::Update(eu) => {
                eu.entry_hash = original_entry_hash.clone();
            }
            _ => {}
        };

        Self {
            signature: fixt!(Signature),
            original_entry,
            new_entry,
            entry_update_header,
            entry_update_entry,
            original_header,
            original_header_hash,
            original_entry_hash,
            entry_delete,
            link_add,
            link_remove,
        }
    }
}

#[derive(Clone)]
enum Db {
    Integrated(DhtOp),
    IntegratedEmpty,
    IntQueue(DhtOp),
    IntQueueEmpty,
    MetaEmpty,
    MetaActivity(Header),
    MetaUpdate(AnyDhtHash, Header),
    MetaDelete(HeaderHash, Header),
    MetaLinkEmpty(CreateLink),
}

impl Db {
    /// Checks that the database is in a state
    #[instrument(skip(expects, env))]
    async fn check(expects: Vec<Self>, env: DbWrite<DbKindDht>, here: String) {
        fresh_reader_test(env, |txn| {
            // print_stmts_test(env, |txn| {
            for expect in expects {
                match expect {
                    Db::Integrated(op) => {
                        let op_hash = DhtOpHash::with_data_sync(&op);

                        let found: bool = txn
                            .query_row(
                                "
                                SELECT EXISTS(
                                    SELECT 1 FROM DhtOP
                                    WHERE when_integrated IS NOT NULL
                                    AND hash = :hash
                                    AND validation_status = :status
                                )
                                ",
                                named_params! {
                                    ":hash": op_hash,
                                    ":status": ValidationStatus::Valid,
                                },
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(found, "{}\n{:?}", here, op);
                    }
                    Db::IntQueue(op) => {
                        let op_hash = DhtOpHash::with_data_sync(&op);

                        let found: bool = txn
                            .query_row(
                                "
                                SELECT EXISTS(
                                    SELECT 1 FROM DhtOP
                                    WHERE when_integrated IS NULL
                                    AND validation_stage = 3
                                    AND hash = :hash
                                    AND validation_status = :status
                                )
                                ",
                                named_params! {
                                    ":hash": op_hash,
                                    ":status": ValidationStatus::Valid,
                                },
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(found, "{}\n{:?}", here, op);
                    }
                    Db::MetaActivity(header) => {
                        let hash = HeaderHash::with_data_sync(&header);
                        let basis: AnyDhtHash = header.author().clone().into();
                        let found: bool = txn
                            .query_row(
                                "
                                SELECT EXISTS(
                                    SELECT 1 FROM DhtOP
                                    WHERE when_integrated IS NOT NULL
                                    AND basis_hash = :basis
                                    AND header_hash = :hash
                                    AND validation_status = :status
                                    AND type = :activity
                                )
                                ",
                                named_params! {
                                    ":basis": basis,
                                    ":hash": hash,
                                    ":status": ValidationStatus::Valid,
                                    ":activity": DhtOpType::RegisterAgentActivity,
                                },
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(found, "{}\n{:?}", here, header);
                    }
                    Db::MetaUpdate(base, header) => {
                        let hash = HeaderHash::with_data_sync(&header);
                        let found: bool = txn
                            .query_row(
                                "
                                SELECT EXISTS(
                                    SELECT 1 FROM DhtOP
                                    WHERE when_integrated IS NOT NULL
                                    AND basis_hash = :basis
                                    AND header_hash = :hash
                                    AND validation_status = :status
                                    AND (type = :update_content OR type = :update_element)
                                )
                                ",
                                named_params! {
                                    ":basis": base,
                                    ":hash": hash,
                                    ":status": ValidationStatus::Valid,
                                    ":update_content": DhtOpType::RegisterUpdatedContent,
                                    ":update_element": DhtOpType::RegisterUpdatedElement,
                                },
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(found, "{}\n{:?}", here, header);
                    }
                    Db::MetaDelete(deleted_header_hash, header) => {
                        let hash = HeaderHash::with_data_sync(&header);
                        let found: bool = txn
                            .query_row(
                                "
                                SELECT EXISTS(
                                    SELECT 1 FROM DhtOP
                                    JOIN Header on DhtOp.header_hash = Header.hash
                                    WHERE when_integrated IS NOT NULL
                                    AND validation_status = :status
                                    AND (
                                        (DhtOp.type = :deleted_entry_header AND Header.deletes_header_hash = :deleted_header_hash)
                                        OR
                                        (DhtOp.type = :deleted_by AND header_hash = :hash)
                                    )
                                )
                                ",
                                named_params! {
                                    ":deleted_header_hash": deleted_header_hash,
                                    ":hash": hash,
                                    ":status": ValidationStatus::Valid,
                                    ":deleted_by": DhtOpType::RegisterDeletedBy,
                                    ":deleted_entry_header": DhtOpType::RegisterDeletedEntryHeader,
                                },
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(found, "{}\n{:?}", here, header);
                    }
                    Db::IntegratedEmpty => {
                        let not_empty: bool = txn
                            .query_row(
                                "SELECT EXISTS(SELECT 1 FROM DhtOP WHERE when_integrated IS NOT NULL)",
                                [],
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(!not_empty, "{}", here);
                    }
                    Db::IntQueueEmpty => {
                        let not_empty: bool = txn
                            .query_row(
                                "SELECT EXISTS(SELECT 1 FROM DhtOP WHERE when_integrated IS NULL)",
                                [],
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(!not_empty, "{}", here);
                    }
                    Db::MetaEmpty => {
                        let not_empty: bool = txn
                            .query_row(
                                "SELECT EXISTS(SELECT 1 FROM DhtOP WHERE when_integrated IS NOT NULL)",
                                [],
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(!not_empty, "{}", here);
                    }
                    Db::MetaLinkEmpty(link_add) => {
                        let query = GetLinksQuery::new(
                            link_add.base_address.clone(),
                            Some(LinkTypeQuery::AllTypes(link_add.zome_id)),
                            Some(link_add.tag.clone()),
                        );
                        let res = query.run(Txn::from(&txn)).unwrap();
                        assert_eq!(res.len(), 0, "{}", here);
                    }
                }
            }
        })
    }

    // Sets the database to a certain state
    #[instrument(skip(pre_state, env))]
    async fn set<'env>(pre_state: Vec<Self>, env: DbWrite<DbKindDht>) {
        env.conn()
            .unwrap()
            .with_commit_sync::<WorkspaceError, _, _>(|txn| {
                for state in pre_state {
                    match state {
                        Db::Integrated(op) => {
                            let op = DhtOpHashed::from_content_sync(op.clone());
                            let hash = op.as_hash().clone();
                            mutations::insert_op(txn, &op).unwrap();
                            mutations::set_when_integrated(txn, &hash, Timestamp::now()).unwrap();
                            mutations::set_validation_status(txn, &hash, ValidationStatus::Valid)
                                .unwrap();
                        }
                        Db::IntQueue(op) => {
                            let op = DhtOpHashed::from_content_sync(op.clone());
                            let hash = op.as_hash().clone();
                            mutations::insert_op(txn, &op).unwrap();
                            mutations::set_validation_stage(
                                txn,
                                &hash,
                                ValidationLimboStatus::AwaitingIntegration,
                            )
                            .unwrap();
                            mutations::set_validation_status(txn, &hash, ValidationStatus::Valid)
                                .unwrap();
                        }
                        _ => {
                            unimplemented!("Use Db::Integrated");
                        }
                    }
                }
                Ok(())
            })
            .unwrap();
    }
}

async fn call_workflow<'env>(env: DbWrite<DbKindDht>) {
    let (qt, _rx) = TriggerSender::new();
    let test_network = test_network(None, None).await;
    let holochain_p2p_cell = test_network.dna_network();
    integrate_dht_ops_workflow(env.clone(), &env.clone().into(), qt, holochain_p2p_cell)
        .await
        .unwrap();
}

// Need to clear the data from the previous test
fn clear_dbs(env: DbWrite<DbKindDht>) {
    env.conn()
        .unwrap()
        .with_commit_sync(|txn| {
            txn.execute("DELETE FROM DhtOP", []).unwrap();
            txn.execute("DELETE FROM Header", []).unwrap();
            txn.execute("DELETE FROM Entry", []).unwrap();
            StateMutationResult::Ok(())
        })
        .unwrap();
}

// TESTS BEGIN HERE
// The following show an op or ops that you want to test
// with a desired pre-state that you want the database in
// and the expected state of the database after the workflow is run

fn register_agent_activity(mut a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    a.link_add.header_seq = 5;
    let dep = DhtOp::RegisterAgentActivity(a.signature.clone(), a.link_add.clone().into());
    let hash = HeaderHash::with_data_sync(&Header::CreateLink(a.link_add.clone()));
    let mut new_header = a.link_add.clone();
    new_header.prev_header = hash;
    new_header.header_seq += 1;
    let op = DhtOp::RegisterAgentActivity(a.signature.clone(), new_header.clone().into());
    let pre_state = vec![Db::Integrated(dep.clone()), Db::IntQueue(op.clone())];
    let expect = vec![
        Db::Integrated(dep.clone()),
        Db::MetaActivity(a.link_add.clone().into()),
        Db::Integrated(op.clone()),
        Db::MetaActivity(new_header.clone().into()),
    ];
    (pre_state, expect, "register agent activity")
}

fn register_updated_element(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = DhtOp::StoreElement(
        a.signature.clone(),
        a.original_header.clone().into(),
        Some(a.original_entry.clone().into()),
    );
    let op = DhtOp::RegisterUpdatedElement(
        a.signature.clone(),
        a.entry_update_header.clone(),
        Some(a.new_entry.clone().into()),
    );
    let pre_state = vec![Db::Integrated(original_op), Db::IntQueue(op.clone())];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaUpdate(
            a.original_header_hash.clone().into(),
            a.entry_update_header.clone().into(),
        ),
    ];
    (pre_state, expect, "register updated element")
}

fn register_replaced_by_for_entry(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = DhtOp::StoreEntry(
        a.signature.clone(),
        a.original_header.clone(),
        a.original_entry.clone().into(),
    );
    let op = DhtOp::RegisterUpdatedContent(
        a.signature.clone(),
        a.entry_update_entry.clone(),
        Some(a.new_entry.clone().into()),
    );
    let pre_state = vec![Db::Integrated(original_op), Db::IntQueue(op.clone())];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaUpdate(
            a.original_entry_hash.clone().into(),
            a.entry_update_entry.clone().into(),
        ),
    ];
    (pre_state, expect, "register replaced by for entry")
}

fn register_deleted_by(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = DhtOp::StoreEntry(
        a.signature.clone(),
        a.original_header.clone(),
        a.original_entry.clone().into(),
    );
    let op = DhtOp::RegisterDeletedEntryHeader(a.signature.clone(), a.entry_delete.clone());
    let pre_state = vec![Db::Integrated(original_op), Db::IntQueue(op.clone())];
    let expect = vec![
        Db::IntQueueEmpty,
        Db::Integrated(op.clone()),
        Db::MetaDelete(
            a.original_header_hash.clone().into(),
            a.entry_delete.clone().into(),
        ),
    ];
    (pre_state, expect, "register deleted by")
}

fn register_deleted_header_by(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = DhtOp::StoreElement(
        a.signature.clone(),
        a.original_header.clone().into(),
        Some(a.original_entry.clone().into()),
    );
    let op = DhtOp::RegisterDeletedBy(a.signature.clone(), a.entry_delete.clone());
    let pre_state = vec![Db::IntQueue(op.clone()), Db::Integrated(original_op)];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaDelete(
            a.original_header_hash.clone().into(),
            a.entry_delete.clone().into(),
        ),
    ];
    (pre_state, expect, "register deleted header by")
}

fn register_delete_link(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = DhtOp::StoreEntry(
        a.signature.clone(),
        a.original_header.clone(),
        a.original_entry.clone().into(),
    );
    let original_link_op = DhtOp::RegisterAddLink(a.signature.clone(), a.link_add.clone());
    let op = DhtOp::RegisterRemoveLink(a.signature.clone(), a.link_remove.clone());
    let pre_state = vec![
        Db::Integrated(original_op),
        Db::Integrated(original_link_op),
        Db::IntQueue(op.clone()),
    ];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaLinkEmpty(a.link_add.clone()),
    ];
    (pre_state, expect, "register link remove")
}

// Link remove when not an author
fn register_delete_link_missing_base(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op = DhtOp::RegisterRemoveLink(a.signature.clone(), a.link_remove.clone());
    let pre_state = vec![Db::IntQueue(op.clone())];
    let expect = vec![Db::IntegratedEmpty, Db::IntQueue(op.clone()), Db::MetaEmpty];
    (
        pre_state,
        expect,
        "register remove link remove missing base",
    )
}

// This runs the above tests
#[tokio::test(flavor = "multi_thread")]
async fn test_ops_state() {
    observability::test_run().ok();
    let test_db = test_dht_db();
    let env = test_db.to_db();

    let tests = [
        register_agent_activity,
        register_replaced_by_for_entry,
        register_updated_element,
        register_deleted_by,
        register_deleted_header_by,
        register_delete_link,
        register_delete_link_missing_base,
    ];

    for t in tests.iter() {
        clear_dbs(env.clone());
        println!("test_ops_state on function {:?}", t);
        let td = TestData::new().await;
        let (pre_state, expect, name) = t(td);
        Db::set(pre_state, env.clone()).await;
        call_workflow(env.clone()).await;
        Db::check(expect, env.clone(), format!("{}: {}", name, here!(""))).await;
    }
}
