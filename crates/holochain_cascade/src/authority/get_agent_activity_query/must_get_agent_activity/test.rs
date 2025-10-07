use super::*;
use crate::test_utils::commit_chain;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_sqlite::prelude::DbKindDht;
use holochain_types::test_utils::chain::*;
use isotest::Iso;
use std::sync::Arc;
use test_case::test_case;
use ::fixt::fixt;
use crate::fixt::ActionHashFixturator;

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
/// Extracts the smallest range from the chain filter
/// and then returns all actions within that range
#[tokio::test(flavor = "multi_thread")]
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
/// Check the query returns the appropriate responses.
#[tokio::test(flavor = "multi_thread")]
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

/// Helper function to create a RegisterAgentActivity
fn create_activity(seq: u32) -> RegisterAgentActivity {
    let mut create = fixt!(Create);
    create.action_seq = seq;

    RegisterAgentActivity {
        action: SignedHashed::new_unchecked(
            Action::Create(create.clone()), 
            fixt!(Signature)
        ),
        cached_entry: None,
    }
}

/// Helper function to create a WarrantOp
fn create_warrant_op() -> WarrantOp {
    let author = fake_agent_pub_key(0);
    Signed::new(
        Warrant::new_now(
            WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp { 
                action_author: author.clone(),
                action: (fixt!(ActionHash), fixt!(Signature)),
                chain_op_type: ChainOpType::RegisterAddLink, 
            }), 
        fake_agent_pub_key(1),
            author,
        ),
        fixt!(Signature)
    ).into()
}

#[test_case(
    vec![
        create_activity(0),
        create_activity(1),
        create_activity(2)
    ]
    => true ; "Complete sequence 0-2")]
#[test_case(
    vec![
        create_activity(0),
        create_activity(2)
    ]
    => false ; "Missing sequence number 1")]
#[test_case(
    vec![
        create_activity(0)
    ]
    => true ; "Single activity")]
fn test_is_activity_complete(activity: Vec<RegisterAgentActivity>) -> bool {
    is_activity_complete(&activity)
}

/// Helper function to create a RegisterAgentActivity with specific hash and prev_action
fn create_activity_with_prev(seq: u32, hash: ActionHash, prev: ActionHash) -> RegisterAgentActivity {
    let mut create = fixt!(Create);
    create.action_seq = seq;
    create.prev_action = prev;

    RegisterAgentActivity {
        action: SignedActionHashed::with_presigned(
            ActionHashed::with_pre_hashed(Action::Create(create), hash),
            fixt!(Signature)
        ),
        cached_entry: None,
    }
}

#[test]
fn is_activity_chained_valid_chain() {
    let hash0 = action_hash(&[0]);
    let hash1 = action_hash(&[1]);
    let hash2 = action_hash(&[2]);

    let activity = vec![
        create_activity_with_prev(0, hash0.clone(), action_hash(&[4])),
        create_activity_with_prev(1, hash1.clone(), hash0.clone()),
        create_activity_with_prev(2, hash2.clone(), hash1.clone()),
    ];

    assert!(is_activity_chained(&activity));
}

#[test]
fn is_activity_chained_bad_prev_hash() {
    let hash0 = action_hash(&[0]);
    let hash1 = action_hash(&[1]);
    let hash2 = action_hash(&[2]);
    let hash_wrong = action_hash(&[99]);

    let activity = vec![
        create_activity_with_prev(0, hash0.clone(), action_hash(&[100])),
        create_activity_with_prev(1, hash1.clone(), hash0.clone()),
        create_activity_with_prev(2, hash2.clone(), hash_wrong), // Broken chain
    ];

    assert!(!is_activity_chained(&activity));
}

#[test]
fn is_activity_chained_single() {
    let hash0 = action_hash(&[0]);
    let activity = vec![
        create_activity_with_prev(0, hash0, action_hash(&[5])),
    ];

    assert!(is_activity_chained(&activity));
}

#[test]
fn is_activity_chained_empty() {
    let activity: Vec<RegisterAgentActivity> = vec![];

    assert!(is_activity_chained(&activity));
}

#[test]
fn exclude_forked_activity_removes_duplicate_seqs() {
    // Construct duplicate activity
    let mut create = fixt!(Create);
    create.action_seq = 1;
    let fork_activity_first = RegisterAgentActivity {
        action: SignedHashed::new_unchecked(
            Action::Create(create.clone()), 
            fixt!(Signature)
        ),
        cached_entry: None,
    };

    let mut activity = vec![
        create_activity(0),
        fork_activity_first.clone(),
        create_activity(1),  // Fork at seq 1
        create_activity(2),
    ];
    exclude_forked_activity(&mut activity);

    assert_eq!(activity.len(), 3);

    // Removes duplicate seqs
    let seqs: Vec<u32> = activity.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![0, 1, 2]);

    // Retains first occurance of duplicate seq
    assert_eq!(activity[1].action.hashed.hash, fork_activity_first.action.hashed.hash);
}

#[test]
fn exclude_forked_activity_multiple_forks() {
    let mut activity = vec![
        create_activity(0),
        create_activity(1),
        create_activity(1), // Fork at seq 1
        create_activity(2),
        create_activity(2), // Fork at seq 2
        create_activity(2), // Another fork at seq 2
        create_activity(3),
    ];
    exclude_forked_activity(&mut activity);

    assert_eq!(activity.len(), 4);
    let seqs: Vec<u32> = activity.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3]);
}

#[test]
fn exclude_forked_activity_no_forks() {
    let mut activity = vec![
        create_activity(0),
        create_activity(1),
        create_activity(2),
    ];
    let original_len = activity.len();
    exclude_forked_activity(&mut activity);

    assert_eq!(activity.len(), original_len);
}

#[test]
fn merge_agent_activity_deduplicates() {
    let duplicate_activity = create_activity(1);

    let activity1 = vec![
        create_activity(0),
        duplicate_activity.clone(),
    ];
    let activity2 = vec![
        duplicate_activity, // Duplicate
        create_activity(2),
    ];
    let activity3 = vec![
        create_activity(3),
    ];

    let merged = merge_agent_activity(vec![activity1, activity2, activity3]);

    assert_eq!(merged.len(), 4);
    let seqs: Vec<u32> = merged.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3]);
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

    let warrants1 = vec![warrant1.clone(), warrant2.clone()];
    let warrants2 = vec![warrant2.clone(), create_warrant_op()];

    let merged = merge_warrants(vec![warrants1, warrants2]);

    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0].to_hash(), warrant1.to_hash());
    assert_eq!(merged[1].to_hash(), warrant2.to_hash());
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

#[test]
fn flatten_deduplicate_merges_and_removes_duplicates() {
    let list1 = vec!["a".to_string(), "b".to_string()];
    let list2 = vec!["b".to_string(), "c".to_string()];
    let list3 = vec!["d".to_string()];

    let result = flatten_deduplicate(vec![list1, list2, list3], |s| s.clone());

    assert_eq!(result.len(), 4);
    assert_eq!(result, vec!["a", "b", "c", "d"]);
}

#[test]
fn flatten_deduplicate_preserves_first_occurrence() {
    let list1 = vec![(1, "first"), (2, "a")];
    let list2 = vec![(1, "second"), (3, "b")]; // (1, "second") should be filtered out

    let result = flatten_deduplicate(vec![list1, list2], |(id, _)| *id);

    assert_eq!(result.len(), 3);
    assert_eq!(result[0], (1, "first")); // First occurrence kept
    assert_eq!(result[1], (2, "a"));
    assert_eq!(result[2], (3, "b"));
}
