use super::*;

use crate::core::queue_consumer::TriggerSender;
use crate::test_utils::test_network;
use ::fixt::prelude::*;
use holochain_state::mutations;
use holochain_state::query::link::{GetLinksFilter, GetLinksQuery};

#[derive(Clone)]
struct TestData {
    signature: Signature,
    original_entry: Entry,
    new_entry: Entry,
    entry_update_action: Update,
    entry_update_entry: Update,
    original_action_hash: ActionHash,
    original_entry_hash: EntryHash,
    original_action: NewEntryAction,
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

    #[cfg_attr(feature = "instrument", tracing::instrument())]
    fn new_inner(original_entry: Entry, new_entry: Entry) -> Self {
        // original entry
        let original_entry_hash =
            EntryHashed::from_content_sync(original_entry.clone()).into_hash();

        // New entry
        let new_entry_hash = EntryHashed::from_content_sync(new_entry.clone()).into_hash();

        // Original entry and action for updates
        let mut original_action = fixt!(NewEntryAction, PublicCurve);
        tracing::debug!(?original_action);

        match &mut original_action {
            NewEntryAction::Create(c) => c.entry_hash = original_entry_hash.clone(),
            NewEntryAction::Update(u) => u.entry_hash = original_entry_hash.clone(),
        }

        let original_action_hash =
            ActionHashed::from_content_sync(original_action.clone()).into_hash();

        // Action for the new entry
        let mut new_entry_action = fixt!(NewEntryAction, PublicCurve);

        // Update to new entry
        match &mut new_entry_action {
            NewEntryAction::Create(c) => c.entry_hash = new_entry_hash.clone(),
            NewEntryAction::Update(u) => u.entry_hash = new_entry_hash.clone(),
        }

        // Entry update for action
        let mut entry_update_action = fixt!(Update, PublicCurve);
        entry_update_action.entry_hash = new_entry_hash.clone();
        entry_update_action.original_action_address = original_action_hash.clone();

        // Entry update for entry
        let mut entry_update_entry = fixt!(Update, PublicCurve);
        entry_update_entry.entry_hash = new_entry_hash.clone();
        entry_update_entry.original_entry_address = original_entry_hash.clone();
        entry_update_entry.original_action_address = original_action_hash.clone();

        // Entry delete
        let mut entry_delete = fixt!(Delete);
        entry_delete.deletes_address = original_action_hash.clone();

        // Link add
        let mut link_add = fixt!(CreateLink);
        link_add.base_address = original_entry_hash.clone().into();
        link_add.target_address = new_entry_hash.clone().into();
        link_add.tag = fixt!(LinkTag);

        let link_add_hash = ActionHashed::from_content_sync(link_add.clone()).into_hash();

        // Link remove
        let mut link_remove = fixt!(DeleteLink);
        link_remove.base_address = original_entry_hash.clone().into();
        link_remove.link_add_address = link_add_hash.clone();

        // Any Action
        let mut any_action = fixt!(Action, PublicCurve);
        match &mut any_action {
            Action::Create(ec) => {
                ec.entry_hash = original_entry_hash.clone();
            }
            Action::Update(eu) => {
                eu.entry_hash = original_entry_hash.clone();
            }
            _ => {}
        };

        Self {
            signature: fixt!(Signature),
            original_entry,
            new_entry,
            entry_update_action,
            entry_update_entry,
            original_action,
            original_action_hash,
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
    MetaActivity(Action),
    MetaUpdate(AnyDhtHash, Action),
    MetaDelete(ActionHash, Action),
    MetaLinkEmpty(CreateLink),
}

impl Db {
    /// Checks that the database is in a state
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(expects, env)))]
    async fn check(expects: Vec<Self>, env: DbWrite<DbKindDht>, here: String) {
        env.read_async(move |txn| -> DatabaseResult<()> {
            for expect in expects {
                match expect {
                    Db::Integrated(op) => {
                        let op_hash = DhtOpHash::with_data_sync(&op);

                        let found: bool = txn
                            .query_row(
                                "
                                SELECT EXISTS(
                                    SELECT 1 FROM DhtOp
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
                                    SELECT 1 FROM DhtOp
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
                    Db::MetaActivity(action) => {
                        let hash = ActionHash::with_data_sync(&action);
                        let basis: AnyDhtHash = action.author().clone().into();
                        let found: bool = txn
                            .query_row(
                                "
                                SELECT EXISTS(
                                    SELECT 1 FROM DhtOp
                                    WHERE when_integrated IS NOT NULL
                                    AND basis_hash = :basis
                                    AND action_hash = :hash
                                    AND validation_status = :status
                                    AND type = :activity
                                )
                                ",
                                named_params! {
                                    ":basis": basis,
                                    ":hash": hash,
                                    ":status": ValidationStatus::Valid,
                                    ":activity": ChainOpType::RegisterAgentActivity,
                                },
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(found, "{}\n{:?}", here, action);
                    }
                    Db::MetaUpdate(base, action) => {
                        let hash = ActionHash::with_data_sync(&action);
                        let found: bool = txn
                            .query_row(
                                "
                                SELECT EXISTS(
                                    SELECT 1 FROM DhtOp
                                    WHERE when_integrated IS NOT NULL
                                    AND basis_hash = :basis
                                    AND action_hash = :hash
                                    AND validation_status = :status
                                    AND (type = :update_content OR type = :update_record)
                                )
                                ",
                                named_params! {
                                    ":basis": base,
                                    ":hash": hash,
                                    ":status": ValidationStatus::Valid,
                                    ":update_content": ChainOpType::RegisterUpdatedContent,
                                    ":update_record": ChainOpType::RegisterUpdatedRecord,
                                },
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(found, "{}\n{:?}", here, action);
                    }
                    Db::MetaDelete(deleted_action_hash, action) => {
                        let hash = ActionHash::with_data_sync(&action);
                        let found: bool = txn
                            .query_row(
                                "
                                SELECT EXISTS(
                                    SELECT 1 FROM DhtOp
                                    JOIN Action on DhtOp.action_hash = Action.hash
                                    WHERE when_integrated IS NOT NULL
                                    AND validation_status = :status
                                    AND (
                                        (DhtOp.type = :deleted_entry_action AND Action.deletes_action_hash = :deleted_action_hash)
                                        OR
                                        (DhtOp.type = :deleted_by AND action_hash = :hash)
                                    )
                                )
                                ",
                                named_params! {
                                    ":deleted_action_hash": deleted_action_hash,
                                    ":hash": hash,
                                    ":status": ValidationStatus::Valid,
                                    ":deleted_by": ChainOpType::RegisterDeletedBy,
                                    ":deleted_entry_action": ChainOpType::RegisterDeletedEntryAction,
                                },
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(found, "{}\n{:?}", here, action);
                    }
                    Db::IntegratedEmpty => {
                        let not_empty: bool = txn
                            .query_row(
                                "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE when_integrated IS NOT NULL)",
                                [],
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(!not_empty, "{}", here);
                    }
                    Db::IntQueueEmpty => {
                        let not_empty: bool = txn
                            .query_row(
                                "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE when_integrated IS NULL)",
                                [],
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(!not_empty, "{}", here);
                    }
                    Db::MetaEmpty => {
                        let not_empty: bool = txn
                            .query_row(
                                "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE when_integrated IS NOT NULL)",
                                [],
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert!(!not_empty, "{}", here);
                    }
                    Db::MetaLinkEmpty(link_add) => {
                        let query = GetLinksQuery::new(
                            link_add.base_address.clone(),
                            LinkTypeFilter::single_type(link_add.zome_index, link_add.link_type),
                            Some(link_add.tag.clone()),
                            GetLinksFilter::default(),
                        );
                        let res = query.run(Txn::from(&txn)).unwrap();
                        assert_eq!(res.len(), 0, "{}", here);
                    }
                }
            }

            Ok(())
        }).await.unwrap();
    }

    // Sets the database to a certain state
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(pre_state, env)))]
    async fn set<'env>(pre_state: Vec<Self>, env: DbWrite<DbKindDht>) {
        env.write_async(move |txn| -> DatabaseResult<()> {
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
                            ValidationStage::AwaitingIntegration,
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
        .await
        .unwrap();
    }
}

#[allow(unused)]
async fn call_workflow<'env>(env: DbWrite<DbKindDht>) {
    let (qt, _rx) = TriggerSender::new();
    let test_network = test_network(None, None).await;
    let holochain_p2p_cell = test_network.dna_network();
    integrate_dht_ops_workflow(env.clone(), env.clone().into(), qt, holochain_p2p_cell)
        .await
        .unwrap();
}

// Need to clear the data from the previous test
#[allow(unused)]
async fn clear_dbs(env: DbWrite<DbKindDht>) {
    env.write_async(move |txn| -> StateMutationResult<()> {
        txn.execute("DELETE FROM DhtOp", []).unwrap();
        txn.execute("DELETE FROM Action", []).unwrap();
        txn.execute("DELETE FROM Entry", []).unwrap();
        Ok(())
    })
    .await
    .unwrap();
}

// TESTS BEGIN HERE
// The following show an op or ops that you want to test
// with a desired pre-state that you want the database in
// and the expected state of the database after the workflow is run
#[allow(unused)] // Wrong detection by Clippy, due to unusual calling pattern
fn register_agent_activity(mut a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    a.link_add.action_seq = 5;
    let dep: DhtOp =
        ChainOp::RegisterAgentActivity(a.signature.clone(), a.link_add.clone().into()).into();
    let hash = ActionHash::with_data_sync(&Action::CreateLink(a.link_add.clone()));
    let mut new_action = a.link_add.clone();
    new_action.prev_action = hash;
    new_action.action_seq += 1;
    let op: DhtOp =
        ChainOp::RegisterAgentActivity(a.signature.clone(), new_action.clone().into()).into();
    let pre_state = vec![Db::Integrated(dep.clone()), Db::IntQueue(op.clone())];
    let expect = vec![
        Db::Integrated(dep.clone()),
        Db::MetaActivity(a.link_add.clone().into()),
        Db::Integrated(op.clone()),
        Db::MetaActivity(new_action.clone().into()),
    ];
    (pre_state, expect, "register agent activity")
}

#[allow(unused)] // Wrong detection by Clippy, due to unusual calling pattern
fn register_updated_record(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = ChainOp::StoreRecord(
        a.signature.clone(),
        a.original_action.clone().into(),
        a.original_entry.clone().into(),
    )
    .into();
    let op: DhtOp = ChainOp::RegisterUpdatedRecord(
        a.signature.clone(),
        a.entry_update_action.clone(),
        a.new_entry.clone().into(),
    )
    .into();
    let pre_state = vec![Db::Integrated(original_op), Db::IntQueue(op.clone())];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaUpdate(
            a.original_action_hash.clone().into(),
            a.entry_update_action.clone().into(),
        ),
    ];
    (pre_state, expect, "register updated record")
}

#[allow(unused)] // Wrong detection by Clippy, due to unusual calling pattern
fn register_replaced_by_for_entry(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = ChainOp::StoreEntry(
        a.signature.clone(),
        a.original_action.clone(),
        a.original_entry.clone(),
    )
    .into();
    let op: DhtOp = ChainOp::RegisterUpdatedContent(
        a.signature.clone(),
        a.entry_update_entry.clone(),
        a.new_entry.clone().into(),
    )
    .into();
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

#[allow(unused)] // Wrong detection by Clippy, due to unusual calling pattern
fn register_deleted_by(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = ChainOp::StoreEntry(
        a.signature.clone(),
        a.original_action.clone(),
        a.original_entry.clone(),
    )
    .into();
    let op: DhtOp =
        ChainOp::RegisterDeletedEntryAction(a.signature.clone(), a.entry_delete.clone()).into();
    let pre_state = vec![Db::Integrated(original_op), Db::IntQueue(op.clone())];
    let expect = vec![
        Db::IntQueueEmpty,
        Db::Integrated(op.clone()),
        Db::MetaDelete(
            a.original_action_hash.clone(),
            a.entry_delete.clone().into(),
        ),
    ];
    (pre_state, expect, "register deleted by")
}

#[allow(unused)] // Wrong detection by Clippy, due to unusual calling pattern
fn register_deleted_action_by(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = ChainOp::StoreRecord(
        a.signature.clone(),
        a.original_action.clone().into(),
        a.original_entry.clone().into(),
    )
    .into();
    let op: DhtOp = ChainOp::RegisterDeletedBy(a.signature.clone(), a.entry_delete.clone()).into();
    let pre_state = vec![Db::IntQueue(op.clone()), Db::Integrated(original_op)];
    let expect = vec![
        Db::Integrated(op.clone()),
        Db::MetaDelete(
            a.original_action_hash.clone(),
            a.entry_delete.clone().into(),
        ),
    ];
    (pre_state, expect, "register deleted action by")
}

#[allow(unused)] // Wrong detection by Clippy, due to unusual calling pattern
fn register_delete_link(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let original_op = ChainOp::StoreEntry(
        a.signature.clone(),
        a.original_action.clone(),
        a.original_entry.clone(),
    )
    .into();
    let original_link_op = ChainOp::RegisterAddLink(a.signature.clone(), a.link_add.clone()).into();
    let op: DhtOp = ChainOp::RegisterRemoveLink(a.signature.clone(), a.link_remove.clone()).into();
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
#[allow(unused)] // Wrong detection by Clippy, due to unusual calling pattern
fn register_delete_link_missing_base(a: TestData) -> (Vec<Db>, Vec<Db>, &'static str) {
    let op: DhtOp = ChainOp::RegisterRemoveLink(a.signature.clone(), a.link_remove.clone()).into();
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
    holochain_trace::test_run();
    let test_db = test_dht_db();
    let env = test_db.to_db();

    let tests = [
        register_agent_activity,
        register_replaced_by_for_entry,
        register_updated_record,
        register_deleted_by,
        register_deleted_action_by,
        register_delete_link,
        register_delete_link_missing_base,
    ];

    for t in tests.iter() {
        clear_dbs(env.clone()).await;
        println!("test_ops_state on function {:?}", t);
        let td = TestData::new().await;
        let (pre_state, expect, name) = t(td);
        Db::set(pre_state, env.clone()).await;
        call_workflow(env.clone()).await;
        Db::check(
            expect,
            env.clone(),
            format!("{}: {}", name, crate::here!("")),
        )
        .await;
    }
}
