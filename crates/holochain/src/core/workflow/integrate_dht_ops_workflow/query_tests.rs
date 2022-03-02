use std::collections::HashMap;
use std::collections::HashSet;

use ::fixt::prelude::*;
use holo_hash::HasHash;
use holochain_sqlite::db::WriteManager;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::prelude::*;
use holochain_types::dht_op::DhtOpHashed;
use holochain_zome_types::fixt::*;

use crate::test_utils::test_network;

use super::*;

struct Expected {
    hashes: HashSet<DhtOpHash>,
    ops: HashMap<DhtOpHash, DhtOpHashed>,
}

struct SharedData {
    seq: u32,
    agent: AgentPubKey,
    prev_hash: HeaderHash,
    last_header: HeaderHash,
    last_entry: EntryHash,
    last_link: HeaderHash,
}
#[derive(Debug, Clone, Copy, Default)]
struct Facts {
    integrated: bool,
    awaiting_integration: bool,
    sequential: bool,
    last_header: bool,
    last_entry: bool,
    last_link: bool,
}
#[derive(Debug, Clone, Copy)]
struct Scenario {
    facts: Facts,
    op: DhtOpType,
}

impl Scenario {
    fn with_dep(op_type: DhtOpType) -> [Self; 2] {
        match op_type {
            DhtOpType::RegisterAgentActivity => {
                let mut dep = Self::without_dep(op_type);
                let mut op = Self::without_dep(op_type);
                dep.facts.integrated = true;
                dep.facts.awaiting_integration = false;
                dep.facts.sequential = true;
                op.facts.sequential = true;
                [dep, op]
            }
            DhtOpType::RegisterDeletedEntryHeader | DhtOpType::RegisterUpdatedContent => {
                let mut dep = Self::without_dep(DhtOpType::StoreEntry);
                let mut op = Self::without_dep(op_type);
                dep.facts.integrated = true;
                dep.facts.awaiting_integration = false;
                op.facts.last_header = true;
                [dep, op]
            }
            DhtOpType::RegisterDeletedBy | DhtOpType::RegisterUpdatedElement => {
                let mut dep = Self::without_dep(DhtOpType::StoreElement);
                let mut op = Self::without_dep(op_type);
                dep.facts.integrated = true;
                dep.facts.awaiting_integration = false;
                op.facts.last_header = true;
                [dep, op]
            }
            DhtOpType::RegisterAddLink => {
                let mut dep = Self::without_dep(DhtOpType::StoreEntry);
                let mut op = Self::without_dep(op_type);
                dep.facts.integrated = true;
                dep.facts.awaiting_integration = false;
                op.facts.last_entry = true;
                [dep, op]
            }
            DhtOpType::RegisterRemoveLink => {
                let mut dep = Self::without_dep(DhtOpType::RegisterAddLink);
                let mut op = Self::without_dep(op_type);
                dep.facts.integrated = true;
                dep.facts.awaiting_integration = false;
                op.facts.last_link = true;
                [dep, op]
            }
            _ => unreachable!("These ops have no dependencies"),
        }
    }

    fn without_dep(op_type: DhtOpType) -> Self {
        Self {
            facts: Facts {
                integrated: false,
                awaiting_integration: true,
                ..Default::default()
            },
            op: op_type,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn integrate_query() {
    observability::test_run().ok();
    let db = test_dht_db();
    let expected = test_data(&db.to_db().into());
    let (qt, _rx) = TriggerSender::new();
    // dump_tmp(&db.db());
    let test_network = test_network(None, None).await;
    let holochain_p2p_cell = test_network.dna_network();
    integrate_dht_ops_workflow(
        db.to_db().into(),
        &db.to_db().into(),
        qt,
        holochain_p2p_cell,
    )
    .await
    .unwrap();
    let hashes = db
        .conn()
        .unwrap()
        .with_reader_test(|txn| {
            let mut stmt =
                txn.prepare("SELECT hash FROM DhtOp WHERE when_integrated IS NOT NULL")?;
            let hashes: HashSet<DhtOpHash> = stmt
                .query_map([], |row| {
                    let hash: DhtOpHash = row.get("hash").unwrap();
                    Ok(hash)
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            DatabaseResult::Ok(hashes)
        })
        .unwrap();
    let diff = hashes.symmetric_difference(&expected.hashes);
    for d in diff {
        tracing::debug!(?d, missing = ?expected.ops.get(d));
    }
    assert_eq!(hashes, expected.hashes);
}

fn create_and_insert_op(
    db: &DbRead<DbKindDht>,
    scenario: Scenario,
    data: &mut SharedData,
) -> DhtOpHashed {
    let Scenario { facts, op } = scenario;
    let entry = matches!(
        op,
        DhtOpType::StoreElement
            | DhtOpType::StoreEntry
            | DhtOpType::RegisterUpdatedContent
            | DhtOpType::RegisterUpdatedElement
    )
    .then(|| Entry::App(fixt!(AppEntryBytes)));

    let seq_not_zero = |seq: &mut u32| {
        if *seq == 0 {
            *seq = 1
        }
    };

    let mut header: Header = match op {
        DhtOpType::RegisterAgentActivity
        | DhtOpType::StoreElement
        | DhtOpType::StoreEntry
        | DhtOpType::RegisterUpdatedContent
        | DhtOpType::RegisterUpdatedElement => {
            let mut update = fixt!(Update);
            seq_not_zero(&mut update.header_seq);
            if facts.last_header {
                update.original_header_address = data.last_header.clone();
            }
            if let Some(entry) = &entry {
                update.entry_hash = EntryHash::with_data_sync(entry);
            }
            data.last_entry = update.entry_hash.clone();
            update.into()
        }
        DhtOpType::RegisterDeletedBy | DhtOpType::RegisterDeletedEntryHeader => {
            let mut delete = fixt!(Delete);
            seq_not_zero(&mut delete.header_seq);
            if facts.last_header {
                delete.deletes_address = data.last_header.clone();
            }
            delete.into()
        }
        DhtOpType::RegisterAddLink => {
            let mut create_link = fixt!(CreateLink);
            seq_not_zero(&mut create_link.header_seq);
            if facts.last_entry {
                create_link.base_address = data.last_entry.clone();
            }
            data.last_link = HeaderHash::with_data_sync(&Header::CreateLink(create_link.clone()));
            create_link.into()
        }
        DhtOpType::RegisterRemoveLink => {
            let mut delete_link = fixt!(DeleteLink);
            seq_not_zero(&mut delete_link.header_seq);
            if facts.last_link {
                delete_link.link_add_address = data.last_link.clone();
            }
            delete_link.into()
        }
    };

    if facts.sequential {
        *header.author_mut() = data.agent.clone();
        *header.header_seq_mut().unwrap() = data.seq;
        *header.prev_header_mut().unwrap() = data.prev_hash.clone();
        data.seq += 1;
        data.prev_hash = HeaderHash::with_data_sync(&header);
    }

    data.last_header = HeaderHash::with_data_sync(&header);
    let state = DhtOpHashed::from_content_sync(
        DhtOp::from_type(op, SignedHeader(header.clone(), fixt!(Signature)), entry).unwrap(),
    );

    db.conn()
        .unwrap()
        .with_commit_sync(|txn| {
            let hash = state.as_hash().clone();
            insert_op(txn, &state).unwrap();
            set_validation_status(txn, &hash, ValidationStatus::Valid).unwrap();
            if facts.integrated {
                set_when_integrated(txn, &hash, holochain_zome_types::Timestamp::now()).unwrap();
            }
            if facts.awaiting_integration {
                set_validation_stage(txn, &hash, ValidationLimboStatus::AwaitingIntegration)
                    .unwrap();
            }
            DatabaseResult::Ok(())
        })
        .unwrap();
    state
}

fn test_data(db: &DbRead<DbKindDht>) -> Expected {
    let mut hashes = HashSet::new();
    let mut ops = HashMap::new();

    let mut data = SharedData {
        seq: 0,
        agent: fixt!(AgentPubKey),
        prev_hash: fixt!(HeaderHash),
        last_header: fixt!(HeaderHash),
        last_entry: fixt!(EntryHash),
        last_link: fixt!(HeaderHash),
    };
    let ops_with_deps = [
        DhtOpType::RegisterAgentActivity,
        DhtOpType::RegisterAddLink,
        DhtOpType::RegisterRemoveLink,
        DhtOpType::RegisterUpdatedContent,
        DhtOpType::RegisterUpdatedElement,
        DhtOpType::RegisterDeletedBy,
        DhtOpType::RegisterDeletedEntryHeader,
    ];
    for op_type in ops_with_deps {
        let scenario = Scenario::without_dep(op_type);
        let op = create_and_insert_op(db, scenario, &mut data);
        ops.insert(op.as_hash().clone(), op);
        let scenarios = Scenario::with_dep(op_type);
        let op = create_and_insert_op(db, scenarios[0], &mut data);
        hashes.insert(op.as_hash().clone());
        ops.insert(op.as_hash().clone(), op);
        let op = create_and_insert_op(db, scenarios[1], &mut data);
        hashes.insert(op.as_hash().clone());
        ops.insert(op.as_hash().clone(), op);
    }
    Expected { hashes, ops }
}
