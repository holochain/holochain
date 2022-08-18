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

#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[8]))
    => agent_chain(&[(0, 0..9)]) ; "Extract full chain")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).take(2)
    => agent_chain(&[(0, 7..9)]) ; "Take 2")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until(action_hash(&[2]))
    => agent_chain(&[(0, 2..9)]) ; "Until 2")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until(action_hash(&[2])).until(action_hash(&[4]))
    => agent_chain(&[(0, 4..9)]) ; "Until 2 Until 4")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until(action_hash(&[2])).until(action_hash(&[4])).take(3)
    => agent_chain(&[(0, 6..9)]) ; "Until 2 Until 4 take 3")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until(action_hash(&[2])).until(action_hash(&[4])).take(1)
    => agent_chain(&[(0, 8..9)]) ; "Until 2 Until 4 take 1")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until(action_hash(&[8])).until(action_hash(&[4])).take(3)
    => agent_chain(&[(0, 8..9)]) ; "Until 8 Until 4 take 3")]
#[tokio::test(flavor = "multi_thread")]
/// Extracts the smallest range from the chain filter
/// and then returns all actions within that range
async fn returns_full_sequence_from_filter(
    chain: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    agent: AgentPubKey,
    filter: ChainFilter,
) -> Vec<(AgentPubKey, Vec<TestChainItem>)> {
    use isotest::Iso;
    let db = commit_chain(chain);
    let data = must_get_agent_activity(db.clone().into(), agent.clone(), filter)
        .await
        .unwrap();
    let data = match data {
        MustGetAgentActivityResponse::Activity(activity) => activity
            .into_iter()
            .map(|RegisterAgentActivity { action: a }| TestChainItem {
                seq: a.hashed.action_seq(),
                hash: TestChainHash::test(a.as_hash()),
                prev: a.hashed.prev_action().map(TestChainHash::test),
            })
            .collect(),
        d @ _ => unreachable!("{:?}", d),
    };
    vec![(agent, data)]
}

#[test_case(
    agent_chain(&[(0, 0..3), (0, 5..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[8]))
    => MustGetAgentActivityResponse::IncompleteChain ; "8 to genesis with 0 till 2 and 5 till 9")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[1]), ChainFilter::new(action_hash(&[8]))
    => MustGetAgentActivityResponse::ChainTopNotFound(action_hash(&[8])) ; "Different agent")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[15]))
    => MustGetAgentActivityResponse::ChainTopNotFound(action_hash(&[15])) ; "Starting chain_top not found")]
#[test_case(
    vec![(agent_hash(&[0]), forked_chain(&[0..6, 3..8]))], agent_hash(&[0]), ChainFilter::new(action_hash(&[7, 1])).take(7)
    => matches MustGetAgentActivityResponse::Activity(a) if a.len() == 7 ; "Handles forks")]
#[test_case(
    agent_chain(&[(0, 0..5)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).until(action_hash(&[2, 1]))
    => matches MustGetAgentActivityResponse::Activity(_) ; "Until hash not found")]
#[test_case(
    vec![(agent_hash(&[0]), forked_chain(&[0..6, 3..8]))], agent_hash(&[0]),
    ChainFilter::new(action_hash(&[5, 0])).until(action_hash(&[4, 1]))
    => MustGetAgentActivityResponse::IncompleteChain ; "Unit hash on fork")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).until(action_hash(&[9]))
    => matches MustGetAgentActivityResponse::Activity(_); "Until is higher then chain_top")]
#[test_case(
    agent_chain(&[(0, 0..2)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[1])).take(0)
    => MustGetAgentActivityResponse::EmptyRange; "Take nothing produces an empty range")]
#[tokio::test(flavor = "multi_thread")]
/// Check the query returns the appropriate responses.
async fn test_responses(
    chain: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    agent: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    let db = commit_chain(chain);
    must_get_agent_activity(db.clone().into(), agent.clone(), filter)
        .await
        .unwrap()
}

fn commit_chain(chain: Vec<(AgentPubKey, Vec<TestChainItem>)>) -> DbWrite<DbKindDht> {
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
