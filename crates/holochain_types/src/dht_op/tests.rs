use crate::fixt::AgentValidationPkgFixturator;
use crate::fixt::CloseChainFixturator;
use crate::fixt::CreateFixturator;
use crate::fixt::CreateLinkFixturator;
use crate::fixt::DeleteLinkFixturator;
use crate::fixt::DnaFixturator;
use crate::fixt::EntryFixturator;
use crate::fixt::EntryHashFixturator;
use crate::fixt::EntryTypeFixturator;
use crate::fixt::InitZomesCompleteFixturator;
use crate::fixt::OpenChainFixturator;
use crate::fixt::UpdateFixturator;
use crate::prelude::*;
use ::fixt::prelude::*;
use arbitrary::Arbitrary;
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::*;
use holochain_trace;
use tracing::*;

use super::OpBasis;

struct RecordTest {
    entry_type: EntryType,
    entry_hash: EntryHash,
    original_entry_hash: EntryHash,
    commons: Box<dyn Iterator<Item = ActionBuilderCommon>>,
    action_hash: ActionHash,
    sig: Signature,
    entry: Entry,
    link_add: CreateLink,
    link_remove: DeleteLink,
    dna: Dna,
    chain_close: CloseChain,
    chain_open: OpenChain,
    agent_validation_pkg: AgentValidationPkg,
    init_zomes_complete: InitZomesComplete,
}

impl RecordTest {
    fn new() -> Self {
        let entry_type = fixt!(EntryType);
        let entry_hash = fixt!(EntryHash);
        let original_entry_hash = fixt!(EntryHash);
        let commons = ActionBuilderCommonFixturator::new(Unpredictable);
        let action_hash = fixt!(ActionHash);
        let sig = fixt!(Signature);
        let entry = fixt!(Entry);
        let link_add = fixt!(CreateLink);
        let link_remove = fixt!(DeleteLink);
        let dna = fixt!(Dna);
        let chain_open = fixt!(OpenChain);
        let chain_close = fixt!(CloseChain);
        let agent_validation_pkg = fixt!(AgentValidationPkg);
        let init_zomes_complete = fixt!(InitZomesComplete);
        Self {
            entry_type,
            entry_hash,
            original_entry_hash,
            commons: Box::new(commons),
            action_hash,
            sig,
            entry,
            link_add,
            link_remove,
            dna,
            chain_close,
            chain_open,
            agent_validation_pkg,
            init_zomes_complete,
        }
    }

    fn create_record(&mut self) -> (Create, Record) {
        let entry_create = builder::Create::new(self.entry_type.clone(), self.entry_hash.clone())
            .build(self.commons.next().unwrap())
            .weightless();
        let record = self.to_record(entry_create.clone().into(), Some(self.entry.clone()));
        (entry_create, record)
    }

    fn update_record(&mut self) -> (Update, Record) {
        let entry_update = builder::Update::new(
            self.original_entry_hash.clone(),
            self.action_hash.clone(),
            self.entry_type.clone(),
            self.entry_hash.clone(),
        )
        .build(self.commons.next().unwrap())
        .weightless();
        let record = self.to_record(entry_update.clone().into(), Some(self.entry.clone()));
        (entry_update, record)
    }

    fn entry_create(&mut self) -> (Record, Vec<ChainOp>) {
        let (entry_create, record) = self.create_record();
        let action: Action = entry_create.clone().into();

        let ops = vec![
            ChainOp::StoreRecord(self.sig.clone(), action.clone(), self.entry.clone().into()),
            ChainOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            ChainOp::StoreEntry(
                self.sig.clone(),
                NewEntryAction::Create(entry_create),
                self.entry.clone(),
            ),
        ];
        (record, ops)
    }

    fn entry_update(&mut self) -> (Record, Vec<ChainOp>) {
        let (entry_update, record) = self.update_record();
        let action: Action = entry_update.clone().into();

        let ops = vec![
            ChainOp::StoreRecord(self.sig.clone(), action.clone(), self.entry.clone().into()),
            ChainOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            ChainOp::StoreEntry(
                self.sig.clone(),
                NewEntryAction::Update(entry_update.clone()),
                self.entry.clone(),
            ),
            ChainOp::RegisterUpdatedContent(
                self.sig.clone(),
                entry_update.clone(),
                self.entry.clone().into(),
            ),
            ChainOp::RegisterUpdatedRecord(
                self.sig.clone(),
                entry_update,
                self.entry.clone().into(),
            ),
        ];
        (record, ops)
    }

    fn entry_delete(&mut self) -> (Record, Vec<ChainOp>) {
        let entry_delete = builder::Delete::new(self.action_hash.clone(), self.entry_hash.clone())
            .build(self.commons.next().unwrap())
            .weightless();
        let record = self.to_record(entry_delete.clone().into(), None);
        let action: Action = entry_delete.clone().into();

        let ops = vec![
            ChainOp::StoreRecord(self.sig.clone(), action.clone(), record.entry().clone()),
            ChainOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            ChainOp::RegisterDeletedBy(self.sig.clone(), entry_delete.clone()),
            ChainOp::RegisterDeletedEntryAction(self.sig.clone(), entry_delete),
        ];
        (record, ops)
    }

    fn link_add(&mut self) -> (Record, Vec<ChainOp>) {
        let record = self.to_record(self.link_add.clone().into(), None);
        let action: Action = self.link_add.clone().into();

        let ops = vec![
            ChainOp::StoreRecord(self.sig.clone(), action.clone(), RecordEntry::NA),
            ChainOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            ChainOp::RegisterAddLink(self.sig.clone(), self.link_add.clone()),
        ];
        (record, ops)
    }

    fn link_remove(&mut self) -> (Record, Vec<ChainOp>) {
        let record = self.to_record(self.link_remove.clone().into(), None);
        let action: Action = self.link_remove.clone().into();

        let ops = vec![
            ChainOp::StoreRecord(self.sig.clone(), action.clone(), RecordEntry::NA),
            ChainOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            ChainOp::RegisterRemoveLink(self.sig.clone(), self.link_remove.clone()),
        ];
        (record, ops)
    }

    fn others(&self) -> Vec<(Record, Vec<ChainOp>)> {
        let records = vec![
            self.to_record(self.dna.clone().into(), None),
            self.to_record(self.chain_open.clone().into(), None),
            self.to_record(self.chain_close.clone().into(), None),
            self.to_record(self.agent_validation_pkg.clone().into(), None),
            self.to_record(self.init_zomes_complete.clone().into(), None),
        ];
        let mut chain_records = Vec::new();
        for record in records {
            let action: Action = record.action().clone();

            let ops = vec![
                ChainOp::StoreRecord(self.sig.clone(), action.clone(), RecordEntry::NA),
                ChainOp::RegisterAgentActivity(self.sig.clone(), action.clone()),
            ];
            chain_records.push((record, ops));
        }
        chain_records
    }

    fn to_record(&self, action: Action, entry: Option<Entry>) -> Record {
        let h = ActionHashed::from_content_sync(action.clone());
        let h = SignedActionHashed::with_presigned(h, self.sig.clone());
        Record::new(h, entry.clone())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_all_ops() {
    holochain_trace::test_run();
    let mut builder = RecordTest::new();
    let (record, expected) = builder.entry_create();
    let result = produce_ops_from_record(&record).unwrap();
    assert_eq!(result, expected);
    let (record, expected) = builder.entry_update();
    let result = produce_ops_from_record(&record).unwrap();
    assert_eq!(result, expected);
    let (record, expected) = builder.entry_delete();
    let result = produce_ops_from_record(&record).unwrap();
    assert_eq!(result, expected);
    let (record, expected) = builder.link_add();
    let result = produce_ops_from_record(&record).unwrap();
    assert_eq!(result, expected);
    let (record, expected) = builder.link_remove();
    let result = produce_ops_from_record(&record).unwrap();
    assert_eq!(result, expected);
    let records = builder.others();
    for (record, expected) in records {
        debug!(?record);
        let result = produce_ops_from_record(&record).unwrap();
        assert_eq!(result, expected);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dht_basis() {
    // Create an action that points to an entry
    let original_action = fixt!(Create);
    let expected_entry_hash: OpBasis = original_action.entry_hash.clone().into();

    let original_action_hash =
        ActionHashed::from_content_sync(Action::Create(original_action.clone()));
    let original_action_hash = original_action_hash.into_inner().1;

    // Create the update action with the same hash
    let update_new_entry = fixt!(Entry);
    let mut entry_update = fixt!(Update, update_new_entry.clone());
    entry_update.original_entry_address = original_action.entry_hash.clone();
    entry_update.original_action_address = original_action_hash;

    // Create the op
    let op =
        ChainOp::RegisterUpdatedContent(fixt!(Signature), entry_update, update_new_entry.into());

    // Get the basis
    let result = op.dht_basis();

    // Check the hash matches
    assert_eq!(expected_entry_hash, result);
}

fn all_records() -> Vec<Record> {
    let mut out = Vec::with_capacity(5);
    let mut builder = RecordTest::new();
    let (record, _) = builder.entry_create();
    out.push(record);
    let (record, _) = builder.entry_update();
    out.push(record);
    let (record, _) = builder.entry_delete();
    out.push(record);
    let (record, _) = builder.link_add();
    out.push(record);
    let (record, _) = builder.link_remove();
    out.push(record);
    out
}

#[test]
fn get_type_op() {
    let check_all_ops = |record| {
        let ops = produce_ops_from_record(&record).unwrap();
        let check_type = |op: ChainOp| {
            let op_type = op.get_type();
            assert_eq!(op.to_lite().get_type(), op_type);
            match op {
                ChainOp::StoreRecord(_, _, _) => assert_eq!(op_type, ChainOpType::StoreRecord),
                ChainOp::StoreEntry(_, _, _) => assert_eq!(op_type, ChainOpType::StoreEntry),
                ChainOp::RegisterAgentActivity(_, _) => {
                    assert_eq!(op_type, ChainOpType::RegisterAgentActivity)
                }
                ChainOp::RegisterUpdatedContent(_, _, _) => {
                    assert_eq!(op_type, ChainOpType::RegisterUpdatedContent)
                }
                ChainOp::RegisterUpdatedRecord(_, _, _) => {
                    assert_eq!(op_type, ChainOpType::RegisterUpdatedRecord)
                }
                ChainOp::RegisterDeletedBy(_, _) => {
                    assert_eq!(op_type, ChainOpType::RegisterDeletedBy)
                }
                ChainOp::RegisterDeletedEntryAction(_, _) => {
                    assert_eq!(op_type, ChainOpType::RegisterDeletedEntryAction)
                }
                ChainOp::RegisterAddLink(_, _) => assert_eq!(op_type, ChainOpType::RegisterAddLink),
                ChainOp::RegisterRemoveLink(_, _) => {
                    assert_eq!(op_type, ChainOpType::RegisterRemoveLink)
                }
            }
        };
        for op in ops {
            check_type(op);
        }
    };

    for record in all_records() {
        check_all_ops(record);
    }
}

#[test]
fn from_type_op() {
    let check_all_ops = |record| {
        let ops = produce_ops_from_record(&record).unwrap();
        let check_identity = |op: ChainOp, action, entry| {
            assert_eq!(
                ChainOp::from_type(op.get_type(), action, entry).unwrap(),
                op
            )
        };
        for op in ops {
            check_identity(
                op,
                SignedAction::from(record.signed_action().clone()),
                record.entry().clone().into_option(),
            );
        }
    };

    for record in all_records() {
        check_all_ops(record);
    }
}

#[test]
fn from_type_op_light() {
    let check_all_ops = |record| {
        let ops = produce_op_lites_from_records(vec![&record]).unwrap();
        let check_identity = |lite: ChainOpLite, action| {
            let action_hash = ActionHash::with_data_sync(action);
            assert_eq!(
                ChainOpLite::from_type(lite.get_type(), action_hash, action).unwrap(),
                lite
            )
        };
        for op in ops {
            check_identity(op, record.action());
        }
    };
    for record in all_records() {
        check_all_ops(record);
    }
}

#[test]
fn test_all_ops_basis() {
    let check_all_ops = |record| {
        let ops = produce_ops_from_record(&record).unwrap();
        let check_basis = |op: ChainOp| match (op.get_type(), op.dht_basis()) {
            (ChainOpType::StoreRecord, basis) => {
                assert_eq!(
                    basis,
                    AnyLinkableHash::from(record.action_address().clone())
                )
            }
            (ChainOpType::StoreEntry, basis) => {
                assert_eq!(
                    basis,
                    AnyLinkableHash::from(record.action().entry_hash().unwrap().clone())
                )
            }
            (ChainOpType::RegisterAgentActivity, basis) => {
                assert_eq!(
                    basis,
                    AnyLinkableHash::from(record.action().author().clone())
                )
            }
            (ChainOpType::RegisterUpdatedContent, basis) => {
                assert_eq!(
                    basis,
                    AnyLinkableHash::from(
                        Update::try_from(record.action().clone())
                            .unwrap()
                            .original_entry_address
                            .clone()
                    )
                )
            }
            (ChainOpType::RegisterUpdatedRecord, basis) => {
                assert_eq!(
                    basis,
                    AnyLinkableHash::from(
                        Update::try_from(record.action().clone())
                            .unwrap()
                            .original_action_address
                            .clone()
                    )
                )
            }
            (ChainOpType::RegisterDeletedBy, basis) => {
                assert_eq!(
                    basis,
                    AnyLinkableHash::from(
                        Delete::try_from(record.action().clone())
                            .unwrap()
                            .deletes_address
                            .clone()
                    )
                )
            }
            (ChainOpType::RegisterDeletedEntryAction, basis) => {
                assert_eq!(
                    basis,
                    AnyLinkableHash::from(
                        Delete::try_from(record.action().clone())
                            .unwrap()
                            .deletes_entry_address
                            .clone()
                    )
                )
            }
            (ChainOpType::RegisterAddLink, basis) => {
                assert_eq!(
                    basis,
                    AnyLinkableHash::from(
                        CreateLink::try_from(record.action().clone())
                            .unwrap()
                            .base_address
                            .clone()
                    )
                )
            }
            (ChainOpType::RegisterRemoveLink, basis) => {
                assert_eq!(
                    basis,
                    AnyLinkableHash::from(
                        DeleteLink::try_from(record.action().clone())
                            .unwrap()
                            .base_address
                            .clone()
                    )
                )
            }
        };
        for op in ops {
            assert_eq!(*op.to_lite().dht_basis(), op.dht_basis());
            check_basis(op);
        }
    };
    for record in all_records() {
        check_all_ops(record);
    }
}

#[test]
fn chain_op_hashes() {
    let mut u = unstructured_noise();
    let chain_op = ChainOp::arbitrary(&mut u).unwrap();
    let dht_op = DhtOp::from(chain_op.clone());
    assert_eq!(chain_op.to_hash(), dht_op.to_hash());
}
