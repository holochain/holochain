use std::collections::HashMap;
use std::collections::HashSet;

use ::fixt::prelude::*;
use holo_hash::HasHash;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::prelude::*;
use holochain_types::dht_op::DhtOpHashed;

use crate::test_utils::test_network;

use super::*;

struct Expected {
    hashes: HashSet<DhtOpHash>,
    ops: HashMap<DhtOpHash, DhtOpHashed>,
}

struct SharedData {
    seq: u32,
    agent: AgentPubKey,
    prev_hash: ActionHash,
    last_action: ActionHash,
    last_entry: EntryHash,
    last_link: ActionHash,
}
#[derive(Debug, Clone, Copy, Default)]
struct Facts {
    integrated: bool,
    awaiting_integration: bool,
    sequential: bool,
    last_action: bool,
    last_entry: bool,
    last_link: bool,
}
#[derive(Debug, Clone, Copy)]
struct Scenario {
    facts: Facts,
    op: ChainOpType,
}

impl Scenario {
    fn with_dep(op_type: ChainOpType) -> [Self; 2] {
        match op_type {
            ChainOpType::RegisterAgentActivity => {
                let mut dep = Self::without_dep(op_type);
                let mut op = Self::without_dep(op_type);
                dep.facts.integrated = true;
                dep.facts.awaiting_integration = false;
                dep.facts.sequential = true;
                op.facts.sequential = true;
                [dep, op]
            }
            ChainOpType::RegisterDeletedEntryAction | ChainOpType::RegisterUpdatedContent => {
                let mut dep = Self::without_dep(ChainOpType::StoreEntry);
                let mut op = Self::without_dep(op_type);
                dep.facts.integrated = true;
                dep.facts.awaiting_integration = false;
                op.facts.last_action = true;
                [dep, op]
            }
            ChainOpType::RegisterDeletedBy | ChainOpType::RegisterUpdatedRecord => {
                let mut dep = Self::without_dep(ChainOpType::StoreRecord);
                let mut op = Self::without_dep(op_type);
                dep.facts.integrated = true;
                dep.facts.awaiting_integration = false;
                op.facts.last_action = true;
                [dep, op]
            }
            ChainOpType::RegisterRemoveLink => {
                let mut dep = Self::without_dep(ChainOpType::RegisterAddLink);
                let mut op = Self::without_dep(op_type);
                dep.facts.integrated = true;
                dep.facts.awaiting_integration = false;
                op.facts.last_link = true;
                [dep, op]
            }
            _ => unreachable!("These ops have no dependencies"),
        }
    }

    fn without_dep(op_type: ChainOpType) -> Self {
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
    holochain_trace::test_run();
    let db = test_dht_db();
    let expected = test_data(&db.to_db()).await;
    let (qt, _rx) = TriggerSender::new();
    let test_network = test_network(None, None).await;
    let holochain_p2p_cell = test_network.dna_network();
    integrate_dht_ops_workflow(db.to_db().into(), db.to_db().into(), qt, holochain_p2p_cell)
        .await
        .unwrap();
    let hashes = db
        .write_async(move |txn| -> DatabaseResult<HashSet<DhtOpHash>> {
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
            Ok(hashes)
        })
        .await
        .unwrap();
    let diff = hashes.symmetric_difference(&expected.hashes);
    for d in diff {
        debug!(?d, missing = ?expected.ops.get(d));
    }
    assert_eq!(hashes, expected.hashes);
}

async fn create_and_insert_op(
    db: &DbWrite<DbKindDht>,
    scenario: Scenario,
    data: &mut SharedData,
) -> DhtOpHashed {
    let Scenario { facts, op } = scenario;
    let entry = matches!(
        op,
        ChainOpType::StoreRecord
            | ChainOpType::StoreEntry
            | ChainOpType::RegisterUpdatedContent
            | ChainOpType::RegisterUpdatedRecord
    )
    .then(|| Entry::App(fixt!(AppEntryBytes)));

    let seq_not_zero = |seq: &mut u32| {
        if *seq == 0 {
            *seq = 1
        }
    };

    let mut action: Action = match op {
        ChainOpType::RegisterAgentActivity
        | ChainOpType::StoreRecord
        | ChainOpType::StoreEntry
        | ChainOpType::RegisterUpdatedContent
        | ChainOpType::RegisterUpdatedRecord => {
            let mut update = fixt!(Update);
            seq_not_zero(&mut update.action_seq);
            if facts.last_action {
                update.original_action_address = data.last_action.clone();
            }
            if let Some(entry) = &entry {
                update.entry_hash = EntryHash::with_data_sync(entry);
            }
            data.last_entry = update.entry_hash.clone();
            update.into()
        }
        ChainOpType::RegisterDeletedBy | ChainOpType::RegisterDeletedEntryAction => {
            let mut delete = fixt!(Delete);
            seq_not_zero(&mut delete.action_seq);
            if facts.last_action {
                delete.deletes_address = data.last_action.clone();
            }
            delete.into()
        }
        ChainOpType::RegisterAddLink => {
            let mut create_link = fixt!(CreateLink);
            seq_not_zero(&mut create_link.action_seq);
            if facts.last_entry {
                create_link.base_address = data.last_entry.clone().into();
            }
            data.last_link = ActionHash::with_data_sync(&Action::CreateLink(create_link.clone()));
            create_link.into()
        }
        ChainOpType::RegisterRemoveLink => {
            let mut delete_link = fixt!(DeleteLink);
            seq_not_zero(&mut delete_link.action_seq);
            if facts.last_link {
                delete_link.link_add_address = data.last_link.clone();
            }
            delete_link.into()
        }
    };

    if facts.sequential {
        *action.author_mut() = data.agent.clone();
        *action.action_seq_mut().unwrap() = data.seq;
        *action.prev_action_mut().unwrap() = data.prev_hash.clone();
        data.seq += 1;
        data.prev_hash = ActionHash::with_data_sync(&action);
    }

    data.last_action = ActionHash::with_data_sync(&action);
    let state = DhtOpHashed::from_content_sync(
        ChainOp::from_type(op, SignedAction(action.clone(), fixt!(Signature)), entry).unwrap(),
    );

    // TODO unexpected write in data setup, was a DbRead
    db.write_async({
        let query_state = state.clone();

        move |txn| -> DatabaseResult<()> {
            let hash = query_state.as_hash().clone();
            insert_op(txn, &query_state).unwrap();
            set_validation_status(txn, &hash, ValidationStatus::Valid).unwrap();
            if facts.integrated {
                set_when_integrated(txn, &hash, holochain_zome_types::prelude::Timestamp::now())
                    .unwrap();
            }
            if facts.awaiting_integration {
                set_validation_stage(txn, &hash, ValidationStage::AwaitingIntegration).unwrap();
            }
            Ok(())
        }
    })
    .await
    .unwrap();

    state
}

async fn test_data(db: &DbWrite<DbKindDht>) -> Expected {
    let mut hashes = HashSet::new();
    let mut ops = HashMap::new();

    let mut data = SharedData {
        seq: 0,
        agent: fixt!(AgentPubKey),
        prev_hash: fixt!(ActionHash),
        last_action: fixt!(ActionHash),
        last_entry: fixt!(EntryHash),
        last_link: fixt!(ActionHash),
    };
    let ops_with_deps = [
        ChainOpType::RegisterAgentActivity,
        ChainOpType::RegisterRemoveLink,
        ChainOpType::RegisterUpdatedContent,
        ChainOpType::RegisterUpdatedRecord,
        ChainOpType::RegisterDeletedBy,
        ChainOpType::RegisterDeletedEntryAction,
    ];
    for op_type in ops_with_deps {
        let scenario = Scenario::without_dep(op_type);
        let op = create_and_insert_op(db, scenario, &mut data).await;
        ops.insert(op.as_hash().clone(), op);
        let scenarios = Scenario::with_dep(op_type);
        let op = create_and_insert_op(db, scenarios[0], &mut data).await;
        hashes.insert(op.as_hash().clone());
        ops.insert(op.as_hash().clone(), op);
        let op = create_and_insert_op(db, scenarios[1], &mut data).await;
        hashes.insert(op.as_hash().clone());
        ops.insert(op.as_hash().clone(), op);
    }
    Expected { hashes, ops }
}
