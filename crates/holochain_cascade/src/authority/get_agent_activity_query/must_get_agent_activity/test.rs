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
        create_activity(3),
        create_activity(2),
        create_activity(1),
        create_activity(0),
    ]
    => true ; "Complete sequence 3-0")]
#[test_case(
    vec![
        create_activity(0)
    ]
    => true ; "Single activity")]
#[test_case(
    vec![]
    => true ; "Empty list")]
#[test_case(
    vec![
        create_activity(2),
        create_activity(0),
    ]
    => false ; "Missing sequence number 1")]
#[test_case(
    vec![
        create_activity(0),
        create_activity(1),
        create_activity(2),
    ]
    => false ; "Not descending order")]
fn test_is_activity_complete_descending(activity: Vec<RegisterAgentActivity>) -> bool {
    is_activity_complete_descending(&activity)
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
fn is_activity_chained_decsending_valid_chain() {
    let hash0 = action_hash(&[0]);
    let hash1 = action_hash(&[1]);
    let hash2 = action_hash(&[2]);

    let activity = vec![
        create_activity_with_prev(2, hash2.clone(), hash1.clone()),
        create_activity_with_prev(1, hash1.clone(), hash0.clone()),
        create_activity_with_prev(0, hash0.clone(), action_hash(&[4])),
    ];

    assert!(is_activity_chained_descending(&activity));
}

#[test]
fn is_activity_chained_descending_single() {
    let hash0 = action_hash(&[0]);
    let activity = vec![
        create_activity_with_prev(0, hash0, action_hash(&[5])),
    ];

    assert!(is_activity_chained_descending(&activity));
}

#[test]
fn is_activity_chained_descending_empty() {
    let activity: Vec<RegisterAgentActivity> = vec![];

    assert!(is_activity_chained_descending(&activity));
}

#[test]
fn is_activity_chained_descending_wrong_prev_hash() {
    let hash0 = action_hash(&[0]);
    let hash1 = action_hash(&[1]);
    let hash2 = action_hash(&[2]);
    let hash_wrong = action_hash(&[99]);

    let activity = vec![
        create_activity_with_prev(2, hash2.clone(), hash1.clone()),
        create_activity_with_prev(1, hash1.clone(), hash_wrong), // Broken chain
        create_activity_with_prev(0, hash0.clone(), action_hash(&[4])),
    ];

    assert!(!is_activity_chained_descending(&activity));
}

#[test]
fn is_activity_chained_descending_wrong_order() {
    let hash0 = action_hash(&[0]);
    let hash1 = action_hash(&[1]);
    let hash2 = action_hash(&[2]);

    let activity = vec![
        create_activity_with_prev(0, hash0.clone(), action_hash(&[4])),
        create_activity_with_prev(0, hash0.clone(), action_hash(&[4])),
        create_activity_with_prev(2, hash2.clone(), hash1.clone()),
    ];

    assert!(!is_activity_chained_descending(&activity));
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
    exclude_forked_activity(&mut activity);

    assert_eq!(activity.len(), 3);
}


#[test]
fn exclude_forked_activity_single_element() {
    let mut activity = vec![
        create_activity(0),
    ];
    exclude_forked_activity(&mut activity);

    assert_eq!(activity.len(), 1);
}


#[test]
fn exclude_forked_activity_empty() {
    let mut activity = vec![];
    exclude_forked_activity(&mut activity);

    assert_eq!(activity.len(), 0);
}

#[test]
fn flatten_deduplicate_sort_merges() {
    let list1 = vec!["a".to_string(), "b".to_string()];
    let list2 = vec!["c".to_string(), "d".to_string()];
    let list3 = vec!["e".to_string(), "f".to_string()];

    let result = flatten_deduplicate_sort(vec![list1, list2, list3], |s| s.clone());

    assert_eq!(result, vec!["a", "b", "c", "d", "e", "f"]);
}

#[test]
fn flatten_deduplicate_sort_merges_and_sorts() {
    let list1 = vec!["c".to_string(), "d".to_string()];
    let list2 = vec!["f".to_string(), "e".to_string()];
    let list3 = vec!["b".to_string(), "a".to_string()];

    let result = flatten_deduplicate_sort(vec![list1, list2, list3], |s| s.clone());

    assert_eq!(result, vec!["a", "b", "c", "d", "e", "f"]);
}

#[test]
fn flatten_deduplicate_sort_merges_sorts_deduplicates() {
    let list1 = vec!["q".to_string(), "d".to_string()];
    let list2 = vec!["f".to_string(), "e".to_string()];
    let list3 = vec!["b".to_string(), "q".to_string()];

    let result = flatten_deduplicate_sort(vec![list1, list2, list3], |s| s.clone());

    assert_eq!(result, vec!["b", "d", "e", "f", "q"]);
}

#[test]
fn merge_agent_activity_deduplicates() {
    let duplicate_activity = create_activity(1);

    let source1 = vec![
        create_activity(0),
        duplicate_activity.clone(),
    ];
    let source2: Vec<RegisterAgentActivity> = vec![
        duplicate_activity, // Duplicate
        create_activity(2),
    ];
    let source3 = vec![
        create_activity(3),
    ];

    let merged = merge_agent_activity(vec![source1, source2, source3]);

    assert_eq!(merged.len(), 4);
    
    let seqs: Vec<u32> = merged.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![3, 2, 1, 0]);
}

#[test]
fn merge_agent_activity_deduplicates_multiple() {
    let duplicate_activity = create_activity(1);
    let duplicate_activity2 = create_activity(4);

    let source1 = vec![
        create_activity(0),
        duplicate_activity.clone(),
        duplicate_activity2.clone(),
    ];
    let source2 = vec![
        duplicate_activity.clone(), // Duplicate
        duplicate_activity2.clone(), // Duplicate 2
        create_activity(2),
    ];
    let source3 = vec![
        create_activity(3),
        duplicate_activity2, // Duplicate 2
    ];

    let merged = merge_agent_activity(vec![source1, source2, source3]);

    assert_eq!(merged.len(), 5);
    
    let seqs: Vec<u32> = merged.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![4, 3, 2, 1, 0]);
}

#[test]
fn merge_agent_activity_sorts_by_action_seq_descending() {
    let source1 = vec![
        create_activity(9),
        create_activity(4),
        create_activity(25),
    ];
    let source2: Vec<RegisterAgentActivity> = vec![
        create_activity(15),
    ];
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
    assert_eq!(merged.iter().filter(|w| w.to_hash() == warrant2.to_hash()).collect::<Vec<_>>().len(), 1);
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
    assert_eq!(merged.iter().filter(|w| w.to_hash() == warrant2.to_hash()).collect::<Vec<_>>().len(), 1);
    assert_eq!(merged.iter().filter(|w| w.to_hash() == warrant3.to_hash()).collect::<Vec<_>>().len(), 1);
}

#[test]
fn merge_warrants_sorts_by_warrant_hash_ascending() {

    let source1 = vec![create_warrant_op(), create_warrant_op(), create_warrant_op()];
    let source2 = vec![create_warrant_op(), create_warrant_op(), create_warrant_op(), create_warrant_op()];
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
