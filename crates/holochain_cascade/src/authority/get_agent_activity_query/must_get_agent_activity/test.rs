use super::*;
use crate::authority::handle_must_get_agent_activity;
use crate::error::CascadeError;
use crate::test_utils::{
    commit_chain, create_activity, create_activity_with_prev, create_warrant_op,
};
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_sqlite::prelude::DbKindDht;
use holochain_types::test_utils::chain::*;
use std::sync::Arc;
use test_case::test_case;

#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[8]))
    => agent_chain(&[(0, 0..9)]) ; "Extract full chain")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::take(action_hash(&[8]), 2)
    => agent_chain(&[(0, 7..9)]) ; "Take 2")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::until_hash(action_hash(&[8]), action_hash(&[2]))
    => agent_chain(&[(0, 2..9)]) ; "Until 2")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[8]), Timestamp::from_micros(2000))
    => agent_chain(&[(0, 2..9)]) ; "Until timestamp 2000")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[8]), Timestamp::from_micros(1999))
    => agent_chain(&[(0, 2..9)]) ; "Until timestamp 1999")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[8]), Timestamp::from_micros(2001))
    => agent_chain(&[(0, 3..9)]) ; "Until timestamp 2001")]
#[test_case(
    agent_chain(&[(0, 4..10)]), agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[9]), Timestamp::from_micros(7000))
    => agent_chain(&[(0, 7..10)]) ; "Until timestamp complete when canonical chain precedes boundary in partial chain")]
#[test_case(
    agent_chain_doubled(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[8]), Timestamp::from_micros(2000))
    => agent_chain_doubled(&[(0, 4..9)]) ; "Until timestamp 2000 doubled")]
#[test_case(
    {
        vec![(
            agent_hash(&[0]),
            vec![
                TestChainItem::with_ts(7, 7000),
                TestChainItem::with_ts(6, 6000),
                TestChainItem::with_ts(5, 3000),
                TestChainItem::with_ts(4, 3000),
                TestChainItem::with_ts(3, 3000),
                TestChainItem::with_ts(2, 2000),
                TestChainItem::with_ts(1, 1000),
                TestChainItem::with_ts(0, 0),
            ],
        )]
    }, agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[7]), Timestamp::from_micros(3000))
    => {
        vec![(
            agent_hash(&[0]),
            vec![
                TestChainItem::with_ts(7, 7000),
                TestChainItem::with_ts(6, 6000),
                TestChainItem::with_ts(5, 3000),
                TestChainItem::with_ts(4, 3000),
                TestChainItem::with_ts(3, 3000),
            ],
        )]
    } ; "Until timestamp includes all identical boundary timestamps")]
#[test_case(
    agent_chain_doubled(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[8]), Timestamp::from_micros(1999))
    => agent_chain_doubled(&[(0, 4..9)]) ; "Until timestamp 1999 doubled")]
#[test_case(
    agent_chain_doubled(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[8]), Timestamp::from_micros(2001))
    => agent_chain_doubled(&[(0, 6..9)]) ; "Until timestamp 2001 doubled")]
/// Returns the expected action range for the provided chain filter.
#[tokio::test(flavor = "multi_thread")]
async fn returns_expected_filtered_sequence_from_filter(
    chain: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    agent: AgentPubKey,
    filter: ChainFilter,
) -> Vec<(AgentPubKey, Vec<TestChainItem>)> {
    let db = commit_chain(
        DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
        chain,
    );
    let data = handle_must_get_agent_activity(db.clone().into(), agent.clone(), filter)
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
                    timestamp: a.action().timestamp(),
                    hash: TestChainHash::from(a.as_hash()),
                    prev: a.hashed.prev_action().map(TestChainHash::from),
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
    vec![(agent_hash(&[0]), forked_chain(&[0..6, 3..8]))], agent_hash(&[0]), ChainFilter::take(action_hash(&[7, 1]), 7)
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7 ; "Excludes forked actions")]
#[test_case(
    agent_chain(&[(0, 0..5)]), agent_hash(&[0]), ChainFilter::until_hash(action_hash(&[4]), action_hash(&[2, 1]))
    => MustGetAgentActivityResponse::UntilHashMissing(action_hash(&[2, 1])) ; "Until hash not found")]
#[test_case(
    vec![(agent_hash(&[0]), forked_chain(&[0..6, 3..8]))], agent_hash(&[0]),
    ChainFilter::until_hash(action_hash(&[5, 0]), action_hash(&[4, 1]))
    => MustGetAgentActivityResponse::UntilHashMissing(action_hash(&[4, 1])) ; "Until hash is on an excluded fork branch")]
#[test_case(
    vec![(agent_hash(&[0]), forked_chain(&[0..6, 3..8]))], agent_hash(&[0]),
    ChainFilter::until_hash(action_hash(&[5, 0]), action_hash(&[4, 0]))
    => matches MustGetAgentActivityResponse::Activity { .. } ; "Until hash is on the retained chain")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[8]), Timestamp::from_micros(9000))
    => MustGetAgentActivityResponse::UntilTimestampGreaterThanChainHead(Timestamp::from_micros(9000)) ; "Until timestamp greater than chain top")]
#[test_case(
    agent_chain(&[(0, 7..10)]), agent_hash(&[0]),
    ChainFilter::until_timestamp(action_hash(&[9]), Timestamp::from_micros(7000))
    => MustGetAgentActivityResponse::UntilTimestampIndeterminate(Timestamp::from_micros(7000)) ; "Until timestamp indeterminate when canonical chain does not precede boundary")]
#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::until_hash(action_hash(&[8]), action_hash(&[9]))
    => MustGetAgentActivityResponse::UntilHashAfterChainHead(action_hash(&[9])) ; "Until hash is after chain top")]
/// Check the query returns the appropriate responses.
#[tokio::test(flavor = "multi_thread")]
async fn handle_must_get_agent_activity_ok(
    chain: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    agent: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    holochain_trace::test_run();
    let db = commit_chain(
        DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
        chain,
    );
    let res = handle_must_get_agent_activity(db.clone().into(), agent.clone(), filter)
        .await
        .unwrap();

    res
}

#[test_case(
    agent_chain(&[(0, 0..10)]), agent_hash(&[0]), ChainFilter::take(action_hash(&[8]), 0)
    => matches CascadeError::InvalidInput(_); "Take is 0")]
#[tokio::test(flavor = "multi_thread")]
async fn handle_must_get_agent_activity_err(
    chain: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    agent: AgentPubKey,
    filter: ChainFilter,
) -> CascadeError {
    holochain_trace::test_run();
    let db = commit_chain(
        DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
        chain,
    );
    let res = handle_must_get_agent_activity(db.clone().into(), agent.clone(), filter)
        .await
        .unwrap_err();

    res
}

#[test]
fn exclude_forked_activity_removes_duplicate_seqs() {
    let hash0 = action_hash(&[0]);
    let hash1 = action_hash(&[1]);
    let hash1_fork = action_hash(&[1, 1]);
    let hash2 = action_hash(&[2]);

    let fork_activity_first = create_activity_with_prev(1, hash1.clone(), hash0.clone());

    // Descending order from chain head.
    let mut activity = vec![
        create_activity_with_prev(2, hash2.clone(), hash1.clone()),
        fork_activity_first.clone(),
        create_activity_with_prev(1, hash1_fork, hash0.clone()), // Fork at seq 1
        create_activity_with_prev(0, hash0.clone(), action_hash(&[9])),
    ];
    exclude_forked_activity(&mut activity, &action_hash(&[2]));

    assert_eq!(activity.len(), 3);

    // Removes duplicate seqs
    let seqs: Vec<u32> = activity.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![2, 1, 0]);

    // The expected hashes are produced, not including the forked action hash
    let hashes: Vec<ActionHash> = activity
        .iter()
        .map(|a| a.action.hashed.hash.clone())
        .collect();
    assert_eq!(hashes, vec![hash2, hash1, hash0]);

    // Retains the fork that is linked to by the head.
    assert_eq!(
        activity[1].action.hashed.hash,
        fork_activity_first.action.hashed.hash
    );
}

#[test]
fn exclude_forked_activity_multiple_forks() {
    let hash0 = action_hash(&[0]);
    let hash1 = action_hash(&[1]);
    let hash1_fork = action_hash(&[1, 1]);
    let hash2 = action_hash(&[2]);
    let hash2_fork = action_hash(&[2, 1]);
    let hash2_fork_2 = action_hash(&[2, 2]);
    let hash3 = action_hash(&[3]);

    let mut activity = vec![
        create_activity_with_prev(3, hash3.clone(), hash2.clone()),
        create_activity_with_prev(2, hash2.clone(), hash1.clone()),
        create_activity_with_prev(2, hash2_fork, hash1.clone()), // Fork at seq 2
        create_activity_with_prev(2, hash2_fork_2, hash1.clone()), // Another fork at seq 2
        create_activity_with_prev(1, hash1.clone(), hash0.clone()),
        create_activity_with_prev(1, hash1_fork, hash0.clone()), // Fork at seq 1
        create_activity_with_prev(0, hash0.clone(), action_hash(&[9])),
    ];
    exclude_forked_activity(&mut activity, &action_hash(&[3]));

    assert_eq!(activity.len(), 4);

    let seqs: Vec<u32> = activity.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![3, 2, 1, 0]);

    let actions: Vec<ActionHash> = activity
        .iter()
        .map(|a| a.action.hashed.hash.clone())
        .collect();
    assert_eq!(actions, vec![hash3, hash2, hash1, hash0]);
}

#[test]
fn exclude_forked_activity_no_forks() {
    let hash0 = action_hash(&[0]);
    let hash1 = action_hash(&[1]);
    let hash2 = action_hash(&[2]);
    let mut activity = vec![
        create_activity_with_prev(2, hash2.clone(), hash1.clone()),
        create_activity_with_prev(1, hash1.clone(), hash0.clone()),
        create_activity_with_prev(0, hash0.clone(), action_hash(&[9])),
    ];
    exclude_forked_activity(&mut activity, &action_hash(&[2]));

    assert_eq!(activity.len(), 3);

    let hashes: Vec<ActionHash> = activity
        .iter()
        .map(|a| a.action.hashed.hash.clone())
        .collect();
    assert_eq!(vec! {hash2, hash1, hash0}, hashes);
}

#[test]
fn exclude_forked_activity_single_element() {
    let mut activity = vec![create_activity_with_prev(
        0,
        action_hash(&[0]),
        action_hash(&[9]),
    )];
    exclude_forked_activity(&mut activity, &action_hash(&[0]));

    assert_eq!(activity.len(), 1);
}

#[test]
fn exclude_forked_activity_empty() {
    let mut activity = vec![];
    exclude_forked_activity(&mut activity, &action_hash(&[0]));

    assert_eq!(activity.len(), 0);
}

#[test_case(
    vec![
        vec!["a".to_string(), "b".to_string()],
        vec!["c".to_string(), "d".to_string()],
        vec!["e".to_string(), "f".to_string()],
    ]
    => vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string(), "e".to_string(), "f".to_string()]; "merges")]
#[test_case(
    vec![
        vec!["c".to_string(), "d".to_string()],
        vec!["f".to_string(), "e".to_string()],
        vec!["b".to_string(), "a".to_string()],
    ]
    => vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string(), "e".to_string(), "f".to_string()]; "merges and sorts")]
#[test_case(
    vec![
        vec!["q".to_string(), "d".to_string()],
        vec!["f".to_string(), "e".to_string()],
        vec!["b".to_string(), "q".to_string()],
    ]
    => vec!["b".to_string(), "d".to_string(), "e".to_string(), "f".to_string(), "q".to_string()]; "merges sorts deduplicates")]
fn flatten_deduplicate_sort_behaves(input: Vec<Vec<String>>) -> Vec<String> {
    flatten_deduplicate_sort(input, |s| s.clone())
}

#[test]
fn merge_agent_activity_deduplicates() {
    let duplicate_activity = create_activity(1);

    let source1 = vec![create_activity(0), duplicate_activity.clone()];
    let source2: Vec<RegisterAgentActivity> = vec![
        duplicate_activity, // Duplicate
        create_activity(2),
    ];
    let source3 = vec![create_activity(3)];

    let merged = merge_agent_activity(vec![source1, source2, source3]);

    assert_eq!(merged.len(), 4);

    let seqs: Vec<u32> = merged.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![3, 2, 1, 0]);
}

#[test]
fn merge_agent_activity_sorts_by_action_seq_descending() {
    let source1 = vec![create_activity(9), create_activity(4), create_activity(25)];
    let source2: Vec<RegisterAgentActivity> = vec![create_activity(15)];
    let source3 = vec![
        create_activity(2),
        create_activity(7),
        create_activity(8),
        create_activity(10),
    ];

    let merged = merge_agent_activity(vec![source1, source2, source3]);

    let action_seqs: Vec<u32> = merged.iter().map(|a| a.action.seq()).collect();
    assert_eq!(action_seqs, vec![25, 15, 10, 9, 8, 7, 4, 2]);
}

#[test]
fn merge_agent_activity_empty_lists() {
    let merged = merge_agent_activity(vec![vec![], vec![], vec![]]);
    assert_eq!(merged.len(), 0);
}

#[test]
fn merge_warrants_deduplicates() {
    let warrant1 = create_warrant_op();
    let warrant2 = create_warrant_op();

    let source1 = vec![warrant1.clone(), warrant2.clone()];
    let source2 = vec![warrant2.clone(), create_warrant_op()];

    let merged = merge_warrants(vec![source1, source2]);

    // Deduplicated
    assert_eq!(merged.len(), 3);

    // Duplicate warrant only occurs once
    assert_eq!(
        merged
            .iter()
            .filter(|w| w.to_hash() == warrant2.to_hash())
            .collect::<Vec<_>>()
            .len(),
        1
    );
}

#[test]
fn merge_warrants_deduplicates_multiple() {
    let warrant1 = create_warrant_op();
    let warrant2 = create_warrant_op();
    let warrant3 = create_warrant_op();

    let source1 = vec![warrant1.clone(), warrant2.clone()];
    let source2 = vec![warrant2.clone(), create_warrant_op(), warrant3.clone()];
    let source3 = vec![warrant3.clone(), create_warrant_op()];

    let merged = merge_warrants(vec![source1, source2, source3]);

    // Deduplicated
    assert_eq!(merged.len(), 5);

    // Duplicate warrants only occurs once
    assert_eq!(
        merged
            .iter()
            .filter(|w| w.to_hash() == warrant2.to_hash())
            .collect::<Vec<_>>()
            .len(),
        1
    );
    assert_eq!(
        merged
            .iter()
            .filter(|w| w.to_hash() == warrant3.to_hash())
            .collect::<Vec<_>>()
            .len(),
        1
    );
}

#[test]
fn merge_warrants_sorts_by_warrant_hash_ascending() {
    let source1 = vec![
        create_warrant_op(),
        create_warrant_op(),
        create_warrant_op(),
    ];
    let source2 = vec![
        create_warrant_op(),
        create_warrant_op(),
        create_warrant_op(),
        create_warrant_op(),
    ];
    let source3 = vec![create_warrant_op(), create_warrant_op()];

    let merged = merge_warrants(vec![source1, source2, source3]);

    // Sorted by hash
    let mut merged_sorted = merged.clone();
    merged_sorted.sort_unstable_by_key(|w| w.to_hash());

    assert_eq!(merged, merged_sorted);
}

#[test]
fn merge_warrants_empty_lists() {
    let merged = merge_warrants(vec![vec![], vec![], vec![]]);
    assert_eq!(merged.len(), 0);
}

#[test]
fn merge_warrants_single_list() {
    let warrant = create_warrant_op();
    let warrants = vec![warrant.clone()];

    let merged = merge_warrants(vec![warrants]);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].to_hash(), warrant.to_hash());
}
