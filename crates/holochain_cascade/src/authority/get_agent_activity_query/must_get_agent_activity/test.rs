use std::sync::Arc;

use super::*;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_sqlite::db::DbKindDht;
use holochain_state::prelude::*;
use holochain_types::dht_op::DhtOpLight;
use holochain_types::dht_op::OpOrder;
use holochain_types::dht_op::UniqueForm;
use holochain_types::test_utils::chain::*;
use holochain_zome_types::ActionRefMut;
use holochain_zome_types::ChainFilter;
use holochain_zome_types::Timestamp;
use test_case::test_case;

#[test_case(agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[9])) => agent_chain(&[(0, 0..10)]))]
#[test_case(agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[9])).take(2) => agent_chain(&[(0, 8..10)]))]
#[test_case(agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[9])).take(2).until(action_hash(&[3])) => agent_chain(&[(0, 8..10)]))]
#[test_case(agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[9])).take(2).until(action_hash(&[9])) => agent_chain(&[(0, 9..10)]))]
#[test_case(agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[9])).take(2).until(action_hash(&[8])) => agent_chain(&[(0, 8..10)]))]
#[test_case(agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[9])).take(5).until(action_hash(&[8])) => agent_chain(&[(0, 8..10)]))]
#[test_case(agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[9])).until(action_hash(&[7])).until(action_hash(&[8])) => agent_chain(&[(0, 8..10)]))]
#[tokio::test(flavor = "multi_thread")]
/// Extracts the largest range from the chain filter
/// and then returns all actions within that range
async fn returns_full_sequence_from_filter(
    chain: Vec<(AgentPubKey, Vec<ChainItem>)>,
    agent: AgentPubKey,
    filter: ChainFilter,
) -> Vec<(AgentPubKey, Vec<ChainItem>)> {
    let db = commit_chain(chain);
    let data = must_get_agent_activity(db.clone().into(), agent.clone(), filter)
        .await
        .unwrap();
    let data = match data {
        MustGetAgentActivityResponse::Activity(activity) => activity
            .into_iter()
            .map(|RegisterAgentActivityOp { action: a }| ChainItem {
                action_seq: a.hashed.action_seq(),
                hash: a.as_hash().clone(),
                prev_action: a.hashed.prev_action().cloned(),
            })
            .collect(),
        d @ _ => unreachable!("{:?}", d),
    };
    vec![(agent, data)]
}

#[test_case(agent_chain(&[(0, 0..3), (0, 5..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[9])) => MustGetAgentActivityResponse::IncompleteChain)]
#[test_case(agent_chain(&[(0, 0..3), (0, 5..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[4])) => MustGetAgentActivityResponse::PositionNotFound)]
#[test_case(agent_chain(&[(0, 0..10)]), agent_hash(&[1]), ChainFilter::new(action_hash(&[4, 1])) => MustGetAgentActivityResponse::PositionNotFound)]
#[tokio::test(flavor = "multi_thread")]
/// Check the query returns the appropriate responses.
async fn test_responses(
    chain: Vec<(AgentPubKey, Vec<ChainItem>)>,
    agent: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    let db = commit_chain(chain);
    must_get_agent_activity(db.clone().into(), agent.clone(), filter)
        .await
        .unwrap()
}

fn commit_chain(chain: Vec<(AgentPubKey, Vec<ChainItem>)>) -> DbWrite<DbKindDht> {
    let data: Vec<_> = chain
        .into_iter()
        .map(|(a, c)| {
            let d = chain_to_ops(c)
                .into_iter()
                .map(|mut op| {
                    *op.action.hashed.content.author_mut() = a.clone();
                    op
                })
                .collect::<Vec<_>>();
            (a, d)
        })
        .collect();
    let db = test_in_mem_db(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))));

    db.test_commit(|txn| {
        for (_, data) in &data {
            for op in data {
                let op_light = DhtOpLight::RegisterAgentActivity(
                    op.action.action_address().clone(),
                    op.action
                        .hashed
                        .entry_hash()
                        .cloned()
                        .unwrap_or_else(|| entry_hash(&[0]))
                        .into(),
                );

                let timestamp = Timestamp::now();
                let (_, hash) =
                    UniqueForm::op_hash(op_light.get_type(), op.action.hashed.content.clone())
                        .unwrap();
                insert_action(txn, &op.action).unwrap();
                insert_op_lite(
                    txn,
                    &op_light,
                    &hash,
                    &OpOrder::new(op_light.get_type(), timestamp),
                    &timestamp,
                )
                .unwrap();
                set_validation_status(txn, &hash, holochain_zome_types::ValidationStatus::Valid)
                    .unwrap();
                set_when_integrated(txn, &hash, Timestamp::now()).unwrap();
            }
        }
    });
    db
}
