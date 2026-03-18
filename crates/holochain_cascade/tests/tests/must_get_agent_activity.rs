use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator};
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_cascade::error::CascadeError;
use holochain_cascade::error::CascadeResult;
use holochain_cascade::test_utils::*;
use holochain_cascade::CascadeImpl;
use holochain_p2p::actor::NetworkRequestOptions;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_state::integrate::insert_locally_validated_op;
use holochain_state::prelude::*;
use holochain_state::query::StateQueryError;
use holochain_types::test_utils::chain::*;
use std::sync::Arc;
use test_case::test_case;

fn warrant(i: u8) -> SignedWarrant {
    let p = WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
        action_author: agent_hash(&[i]),
        action: (::fixt::fixt!(ActionHash), ::fixt::fixt!(Signature)),
        chain_op_type: ChainOpType::StoreRecord,
    });
    let warrant = Warrant::new(
        p,
        ::fixt::fixt!(AgentPubKey),
        Timestamp::now(),
        agent_hash(&[i]),
    );
    SignedWarrant::new(warrant, ::fixt::fixt!(Signature))
}

#[derive(Default, Debug, Clone)]
struct FixtureDataStores {
    scratch: FixtureData,
    authored: FixtureData,
    cache: FixtureData,
    dht: FixtureData,
}

#[derive(Default, Debug, Clone)]
struct FixtureData {
    activity: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    warrants: Vec<SignedWarrant>,
}

// Test cases for must_get_agent_activity behavior across stores and filters.
//
// These cases cover:
// - chain reconstruction from each store and combinations of stores,
// - deduping and fork pruning behavior,
// - until_hash / until_timestamp / take semantics,
// - warrant collection from DB + scratch.
#[test_case(
    FixtureDataStores {
        dht: FixtureData {
            activity: agent_chain(&[(0, 0..3)]),
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]),
    ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with dht 0 till 2")]
#[test_case(
    FixtureDataStores {
        cache: FixtureData {
            activity: agent_chain(&[(0, 0..3)]), ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]),
    ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with cache 0 till 2")]
#[test_case(
    FixtureDataStores {
        authored: FixtureData {
            activity: agent_chain(&[(0, 0..6)]),
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::take(action_hash(&[4]), 4)
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "4 take 4 until 0 with authored 0 till 5")]
#[test_case(
    FixtureDataStores {
        scratch: FixtureData {
            activity: agent_chain(&[(0, 0..3)]),
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with scratch 0 till 2")]
#[test_case(
    FixtureDataStores {
        authored: FixtureData {
            activity: agent_chain(&[(0, 0..3)]),
            ..Default::default()
        },
        scratch:  FixtureData {
            activity: agent_chain(&[(0, 3..6)]),
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::take(action_hash(&[4]), 4)
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "4 take 4 until 0 with authored 0 till 2 and scratch 3 till 5")]
#[test_case(
    FixtureDataStores {
        dht:  FixtureData {
            activity: agent_chain(&[(0, 0..3)]),
            ..Default::default()
        },
        cache:  FixtureData {
            activity: agent_chain(&[(0, 3..6)]),
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::take(action_hash(&[4]), 4)
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "4 take 4 until 0 with dht 0 till 2 and cache 3 till 5")]
#[test_case(
    FixtureDataStores {
        dht:  FixtureData {
            activity: agent_chain(&[(0, 0..7)]),
            ..Default::default()
        },
        cache:  FixtureData {
            activity: agent_chain(&[(1, 1..4)]),
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "4 to genesis with dht 0 till 7 and cache forked 1 till 4")]
#[test_case(
FixtureDataStores {
    authored:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[4]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "4 to genesis with dht 0 till 7 and authored 0 till 7")]
#[test_case(
FixtureDataStores {
    authored:  FixtureData {
            activity: agent_chain(&[(0, 0..7)]),
            ..Default::default()
    },
    dht:  FixtureData {
            activity: agent_chain(&[(0, 0..7)]),
            ..Default::default()
    },
    scratch:  FixtureData {
            activity: agent_chain(&[(0, 7..9)]),
            ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[8]), 4)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "8 take 4 with dht 0 till 7 and authored 0 till 7 and scratch 7 till 9")]
#[test_case(
    FixtureDataStores {
        dht:  FixtureData {
            activity: agent_chain(&[(0, 0..7)]),
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::take(action_hash(&[5]), 1)
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 1; "take 1 returns only chain_top")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: vec![(agent_hash(&[0]), forked_chain(&[5..9, 0..7]))],
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[6]), 7)
=> matches MustGetAgentActivityResponse::IncompleteChain; "forked actions within 1 db, excludes forked actions from chain without chain top")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: vec![(agent_hash(&[0]), forked_chain(&[0..6, 3..8]))],
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(TestChainHash::forked(7, 1).into())
=> matches MustGetAgentActivityResponse::Activity { activity, ..} if activity.len() == 8; "forked actions within 1 db, chain top on retained forked actions")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: vec![(agent_hash(&[0]), forked_chain(&[5..9, 0..7]))],
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(TestChainHash::forked(8, 0).into())
=> matches MustGetAgentActivityResponse::IncompleteChain; "forked actions within 1 db, chain top on excluded forked actions")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    cache:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[5]), 5)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "duplicates between 2 dbs")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    scratch:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[5]), 5)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "duplicates between db and scratch")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::until_timestamp(action_hash(&[6]), Timestamp::from_micros(0))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "until_timestamp before earliest action timestamp")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::until_timestamp(action_hash(&[6]), Timestamp::from_micros(7_i64 * 1000))
=> MustGetAgentActivityResponse::UntilTimestampIndeterminate(Timestamp::from_micros(7_i64 * 1000)); "until_timestamp not found")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 7..10)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::until_timestamp(action_hash(&[9]), Timestamp::from_micros(7_i64 * 1000))
=> MustGetAgentActivityResponse::UntilTimestampIndeterminate(Timestamp::from_micros(7_i64 * 1000)); "until_timestamp missing when lower-bound witness absent")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 4..10)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::until_timestamp(action_hash(&[9]), Timestamp::from_micros(7_i64 * 1000))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 3; "until_timestamp complete with lower-bound witness")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[6]), 10)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "take is greater than total len, take full len")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::until_hash(action_hash(&[6]), action_hash(&[15]))
=> MustGetAgentActivityResponse::UntilHashMissing(action_hash(&[15])); "until_hash is not found")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 5..7)]),
        ..Default::default()
    },
    cache:  FixtureData {
        activity: agent_chain(&[(0, 0..5)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "chain top in dht db, additional activity below chain top cache db ")]
#[test_case(
FixtureDataStores {
    scratch:  FixtureData {
        activity: agent_chain(&[(0, 5..7)]),
        ..Default::default()
    },
    cache:  FixtureData {
        activity: agent_chain(&[(0, 0..5)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "chain top in scratch, additional activity below chain top in other db")]
#[test_case(
FixtureDataStores {
    cache:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[8]))
=> matches MustGetAgentActivityResponse::ChainTopNotFound(_); "chain top not found")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..3)]),
        ..Default::default()
    },
    cache:  FixtureData {
        activity: agent_chain(&[(0, 5..8)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[7]))
=> matches MustGetAgentActivityResponse::IncompleteChain; "multiple dbs with gaps in sequence")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..5)]),
        ..Default::default()
    },
    cache:  FixtureData {
            activity: agent_chain(&[(0, 3..8)]),
                    ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[7]), 3)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 3; "take limit re-applied after merge from multiple dbs")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 5..10)]),
        ..Default::default()
    },
    cache:  FixtureData {
        activity: agent_chain(&[(0, 0..5)]),
        ..Default::default()
    },
    scratch:  FixtureData {
        activity: agent_chain(&[(0, 10..13)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[12]), 5)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "chain top in scratch with history split across multiple dbs")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..10)]),
        ..Default::default()
    },
    cache:  FixtureData {
        activity: agent_chain(&[(0, 0..10)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[9]), 20)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 10; "take larger than available deduplicates across dbs")]
#[test_case(
FixtureDataStores {
    authored:  FixtureData {
        activity: agent_chain(&[(0, 0..5)]),
        ..Default::default()
    },
    dht:  FixtureData {
        activity: agent_chain(&[(0, 3..8)]),
        ..Default::default()
    },
    cache:  FixtureData {
        activity: agent_chain(&[(0, 6..12)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[11]), 6)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 6; "activity from all stores with overlaps")]
#[test_case(
    FixtureDataStores {
        dht: FixtureData {
            activity: agent_chain(&[(0, 0..10)]),
            warrants: vec![warrant(0), warrant(0)],
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::take(action_hash(&[5]), 3)
    => matches MustGetAgentActivityResponse::Activity { warrants, .. } if warrants.len() == 2; "warrants returned outside filter range")]
#[test_case(
    FixtureDataStores {
        dht: FixtureData {
            activity: agent_chain(&[(0, 0..10)]),
            ..Default::default()
        },
        scratch: FixtureData {
            warrants: vec![warrant(0)],
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::until_hash(action_hash(&[8]), action_hash(&[5]))
    => matches MustGetAgentActivityResponse::Activity { warrants, .. } if warrants.len() == 1; "warrants returned from scratch")]
#[test_case(
    FixtureDataStores {
        dht: FixtureData {
            activity: agent_chain(&[(0, 0..10)]),
            warrants: vec![warrant(0)],
        },
        scratch: FixtureData {
            warrants: vec![warrant(0)],
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::until_hash(action_hash(&[8]), action_hash(&[5]))
    => matches MustGetAgentActivityResponse::Activity { warrants, .. } if warrants.len() == 2; "warrants returned from all stores")]
#[test_case(
    FixtureDataStores {
        dht: FixtureData {
            activity: agent_chain(&[(0, 0..3)]),
            warrants: vec![warrant(0)],
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with dht 0 till 2, warrant outside filtered action range")]
#[test_case(
    FixtureDataStores {
        dht: FixtureData {
            activity: agent_chain(&[(0, 0..3)]),
            warrants: vec![warrant(0)],
        },
        ..Default::default()},
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { warrants, .. } if warrants.len() == 1; "1 to genesis with dht 0 till 2 with 1 warrant")]
#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_expected_response(
    data: FixtureDataStores,
    author: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_forks_split_between_2_dbs_keeps_chain_top_branch() {
    holochain_trace::test_run();

    let author = agent_hash(&[0]);
    let chain1 = agent_chain(&[(0, 0..7)]);
    let chain1_fork = vec![
        (author.clone(), vec![TestChainItem::forked(3, 1, 0)]),
        (author.clone(), vec![TestChainItem::forked(4, 1, 0)]),
        (author.clone(), vec![TestChainItem::forked(5, 1, 0)]),
        (author.clone(), vec![TestChainItem::forked(6, 1, 0)]),
        (author.clone(), vec![TestChainItem::forked(7, 1, 0)]),
    ];

    let data = FixtureDataStores {
        dht: FixtureData {
            activity: chain1,
            ..Default::default()
        },
        cache: FixtureData {
            activity: chain1_fork,
            ..Default::default()
        },
        ..Default::default()
    };
    let filter = ChainFilter::take(action_hash(&[5]), 5);

    let res = test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap();

    let activity = match res {
        MustGetAgentActivityResponse::Activity { activity, .. } => activity,
        _ => panic!("Expected Activity response"),
    };

    let seqs: Vec<u32> = activity.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![5, 4, 3, 2, 1]);

    let seq_3_hash = activity
        .iter()
        .find(|a| a.action.seq() == 3)
        .map(|a| a.action.hashed.hash.clone())
        .expect("Expected action with seq 3");
    assert_eq!(seq_3_hash, action_hash(&[3]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_forks_split_between_db_and_scratch_keeps_chain_top_branch() {
    holochain_trace::test_run();
    let author = agent_hash(&[0]);
    let chain1 = agent_chain(&[(0, 0..7)]);
    let chain1_fork = vec![
        (author.clone(), vec![TestChainItem::forked(3, 1, 0)]),
        (author.clone(), vec![TestChainItem::forked(4, 1, 0)]),
        (author.clone(), vec![TestChainItem::forked(5, 1, 0)]),
        (author.clone(), vec![TestChainItem::forked(6, 1, 0)]),
        (author.clone(), vec![TestChainItem::forked(7, 1, 0)]),
    ];

    let data = FixtureDataStores {
        dht: FixtureData {
            activity: chain1,
            ..Default::default()
        },
        scratch: FixtureData {
            activity: chain1_fork,
            ..Default::default()
        },
        ..Default::default()
    };
    let filter = ChainFilter::take(action_hash(&[5]), 5);

    let res = test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap();

    let activity = match res {
        MustGetAgentActivityResponse::Activity { activity, .. } => activity,
        _ => panic!("Expected Activity response"),
    };

    let seqs: Vec<u32> = activity.iter().map(|a| a.action.seq()).collect();
    assert_eq!(seqs, vec![5, 4, 3, 2, 1]);

    let seq_3_hash = activity
        .iter()
        .find(|a| a.action.seq() == 3)
        .map(|a| a.action.hashed.hash.clone())
        .expect("Expected action with seq 3");
    assert_eq!(seq_3_hash, action_hash(&[3]));
}

// Invalid filter input and invalid chain-boundary requests.
#[test_case(
FixtureDataStores {
    dht: FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::until_hash(action_hash(&[3]), action_hash(&[6]))
=> matches CascadeError::QueryError(StateQueryError::InvalidInput(_)); "until hash greater than chain top")]
#[test_case(
FixtureDataStores {
    dht: FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::take(action_hash(&[8]), 0)
=> matches CascadeError::InvalidInput(_); "take 0 invalid input")]
#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_err(
    data: FixtureDataStores,
    author: AgentPubKey,
    filter: ChainFilter,
) -> CascadeError {
    test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap_err()
}

async fn test_must_get_agent_activity_inner(
    data: FixtureDataStores,
    author: AgentPubKey,
    filter: ChainFilter,
) -> CascadeResult<MustGetAgentActivityResponse> {
    let FixtureDataStores {
        scratch: fixture_data_scratch,
        authored: fixture_data_authored,
        cache: fixture_data_cache,
        dht: fixture_data_dht,
    } = data;
    // Write activity to stores
    let dht = commit_chain(
        DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
        fixture_data_dht.activity,
    );
    let cache = commit_chain(
        DbKindCache(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
        fixture_data_cache.activity,
    );
    let authored = commit_chain(
        DbKindAuthored(Arc::new(CellId::new(
            DnaHash::from_raw_36(vec![0; 36]),
            AgentPubKey::from_raw_36(vec![0; 36]),
        ))),
        fixture_data_authored.activity,
    );
    let sync_scratch = Scratch::new().into_sync();
    commit_scratch(sync_scratch.clone(), fixture_data_scratch.activity);

    // Write warrants to stores, excluding cache which never stores warrants
    authored.test_write(move |txn| {
        for w in fixture_data_authored.warrants.clone() {
            let w = DhtOp::from(w).into_hashed();
            insert_op_authored(txn, &w).unwrap();
        }
    });
    dht.test_write(move |txn| {
        for w in fixture_data_dht.warrants.clone() {
            insert_locally_validated_op(txn, DhtOpHashed::from_content_sync(w)).unwrap();
        }
    });
    add_warrants_scratch(sync_scratch.clone(), fixture_data_scratch.warrants);

    // Construct cascade
    let cascade = CascadeImpl::empty()
        .with_authored(authored.into())
        .with_cache(cache)
        .with_dht(dht.into())
        .with_scratch(sync_scratch);

    cascade
        .must_get_agent_activity(author, filter, NetworkRequestOptions::default())
        .await
}

fn add_warrants_scratch(sync_scratch: SyncScratch, warrants: Vec<SignedWarrant>) {
    sync_scratch
        .apply(|scratch| {
            for warrant in warrants {
                scratch.add_warrant(warrant);
            }
        })
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_deterministically_retain_forked_action_on_chain_top_branch() {
    holochain_trace::test_run();

    // Create two forked actions with action_seq 3.
    // The retained action is the one reachable from the provided chain_top.
    let author = agent_hash(&[0]);
    let chain = vec![(author.clone(), forked_chain(&[0..5, 3..6]))];

    let data = FixtureDataStores {
        dht: FixtureData {
            activity: chain,
            ..Default::default()
        },
        ..Default::default()
    };

    // Run must_get_agent_activity using chain head of fork chain 1
    let res = test_must_get_agent_activity_inner(
        data,
        author,
        ChainFilter::new(TestChainHash::forked(5, 1).into()),
    )
    .await
    .unwrap();

    // Select the activity with action_seq 3
    let activity = match res {
        MustGetAgentActivityResponse::Activity { activity, .. } => activity,
        _ => panic!("Expected Activity response"),
    };
    let activity_with_action_seq_3 = activity
        .iter()
        .find(|a| a.action.seq() == 3)
        .expect("Should have action with action_seq 3");

    // The retained activity at seq 3 should be from chain_top's branch.
    assert_eq!(
        activity_with_action_seq_3.action.hashed.hash,
        TestChainHash::forked(3, 1).into(),
        "Expected fork on chain_top branch to be retained"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_until_timestamp_includes_all_identical_boundary_timestamps_across_stores() {
    holochain_trace::test_run();

    let author = agent_hash(&[0]);

    let mut items = chain(0..9);
    for item in &mut items {
        item.timestamp = match item.seq {
            8 => Timestamp::from_micros(8000),
            7 => Timestamp::from_micros(7000),
            6 => Timestamp::from_micros(6000),
            3..=5 => Timestamp::from_micros(3000),
            2 => Timestamp::from_micros(2000),
            1 => Timestamp::from_micros(1000),
            0 => Timestamp::from_micros(0),
            _ => unreachable!(),
        };
    }

    let scratch_items: Vec<_> = items.iter().filter(|i| i.seq >= 5).cloned().collect();
    let dht_items: Vec<_> = items.iter().filter(|i| i.seq <= 4).cloned().collect();

    let data = FixtureDataStores {
        scratch: FixtureData {
            activity: vec![(author.clone(), scratch_items)],
            ..Default::default()
        },
        dht: FixtureData {
            activity: vec![(author.clone(), dht_items)],
            ..Default::default()
        },
        ..Default::default()
    };

    let response = test_must_get_agent_activity_inner(
        data,
        author,
        ChainFilter::until_timestamp(action_hash(&[8]), Timestamp::from_micros(3000)),
    )
    .await
    .unwrap();

    let activity = match response {
        MustGetAgentActivityResponse::Activity { activity, .. } => activity,
        other => panic!("Expected Activity response, got {other:?}"),
    };

    assert_eq!(activity.len(), 6);
    let boundary_count = activity
        .iter()
        .filter(|a| a.action.action().timestamp() == Timestamp::from_micros(3000))
        .count();
    assert_eq!(boundary_count, 3);
}
