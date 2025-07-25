use std::sync::Arc;

use crate::test_utils::commit_chain;

use super::*;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_sqlite::prelude::DbKindDht;
use holochain_types::test_utils::chain::*;
use isotest::Iso;
use test_case::test_case;

#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[8]))
    => agent_chain(&[(0, 0..9)]) ; "Extract full chain")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).take(2)
    => agent_chain(&[(0, 7..9)]) ; "Take 2")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[2]))
    => agent_chain(&[(0, 2..9)]) ; "Until 2")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(2000))
    => agent_chain(&[(0, 2..9)]) ; "Until timestamp 2000")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(1999))
    => agent_chain(&[(0, 2..9)]) ; "Until timestamp 1999")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(2001))
    => agent_chain(&[(0, 3..9)]) ; "Until timestamp 2001")]
#[test_case(
    agent_chain_doubled(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(2000))
    => agent_chain(&[(0, 4..9)]) ; "Until timestamp 2000 doubled")]
#[test_case(
    agent_chain_doubled(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(1999))
    => agent_chain(&[(0, 4..9)]) ; "Until timestamp 1999 doubled")]
#[test_case(
    agent_chain_doubled(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(2001))
    => agent_chain(&[(0, 6..9)]) ; "Until timestamp 2001 doubled")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[2])).until_hash(action_hash(&[4]))
    => agent_chain(&[(0, 4..9)]) ; "Until 2 Until 4")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(2000)).until_timestamp(Timestamp::from_micros(4000))
    => agent_chain(&[(0, 4..9)]) ; "Until timestamp 2 Until timestamp 4")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(2000)).until_hash(action_hash(&[4]))
    => agent_chain(&[(0, 4..9)]) ; "Until timestamp 2 Until 4")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[2])).until_timestamp(Timestamp::from_micros(4000))
    => agent_chain(&[(0, 4..9)]) ; "Until 2 Until timestamp 4")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[2])).until_hash(action_hash(&[4])).take(3)
    => agent_chain(&[(0, 6..9)]) ; "Until 2 Until 4 take 3")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(2000)).until_timestamp(Timestamp::from_micros(4000)).take(3)
    => agent_chain(&[(0, 6..9)]) ; "Until timestamp 2 Until timestamp 4 take 3")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[2])).until_hash(action_hash(&[4])).take(1)
    => agent_chain(&[(0, 8..9)]) ; "Until 2 Until 4 take 1")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[8])).until_hash(action_hash(&[4])).take(3)
    => agent_chain(&[(0, 8..9)]) ; "Until 8 Until 4 take 3")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_timestamp(Timestamp::from_micros(8000)).until_hash(action_hash(&[4])).take(3)
    => agent_chain(&[(0, 8..9)]) ; "Until timestamp 8 Until 4 take 3")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[8])).until_timestamp(Timestamp::from_micros(4000)).take(3)
    => agent_chain(&[(0, 8..9)]) ; "Until 8 Until timestamp 4 take 3")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[3])).until_timestamp(Timestamp::from_micros(4000)).take(8)
    => agent_chain(&[(0, 4..9)]) ; "Until 3 Until timestamp 4 take 8")]
#[tokio::test(flavor = "multi_thread")]
/// Extracts the smallest range from the chain filter
/// and then returns all actions within that range
async fn returns_full_sequence_from_filter(
    chain: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    agent: AgentPubKey,
    filter: ChainFilter,
) -> Vec<(AgentPubKey, Vec<TestChainItem>)> {
    let db = commit_chain(
        DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
        chain,
    );
    let data = must_get_agent_activity(db.clone().into(), agent.clone(), filter)
        .await
        .unwrap();
    let data = match data {
        MustGetAgentActivityResponse::Activity { activity, .. } => activity
            .into_iter()
            .map(
                |RegisterAgentActivity {
                     action: a,
                     cached_entry: _,
                 }| TestChainItem {
                    seq: a.hashed.action_seq(),
                    timestamp: Timestamp(a.hashed.action_seq() as i64 * 1000),
                    hash: TestChainHash::test(a.as_hash()),
                    prev: a.hashed.prev_action().map(TestChainHash::test),
                },
            )
            .collect(),
        d => unreachable!("{:?}", d),
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
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7 ; "Handles forks")]
#[test_case(
    agent_chain(&[(0, 0..5)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).until_hash(action_hash(&[2, 1]))
    => matches MustGetAgentActivityResponse::Activity { .. } ; "Until hash not found")]
#[test_case(
    vec![(agent_hash(&[0]), forked_chain(&[0..6, 3..8]))], agent_hash(&[0]),
    ChainFilter::new(action_hash(&[5, 0])).until_hash(action_hash(&[4, 1]))
    => MustGetAgentActivityResponse::IncompleteChain ; "Unit hash on fork")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[9]))
    => matches MustGetAgentActivityResponse::Activity { .. }; "Until is higher then chain_top")]
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
    let db = commit_chain(
        DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
        chain,
    );
    must_get_agent_activity(db.clone().into(), agent.clone(), filter)
        .await
        .unwrap()
}
