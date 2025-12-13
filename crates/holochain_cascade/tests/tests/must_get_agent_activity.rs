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
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until_hash(action_hash(&[0]))
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
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until_hash(action_hash(&[0]))
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
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until_hash(action_hash(&[0]))
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
agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).take(4)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "8 take 4 with dht 0 till 7 and authored 0 till 7 and scratch 7 till 9")]
#[test_case(
    FixtureDataStores {
        dht:  FixtureData {
            activity: agent_chain(&[(0, 0..7)]),
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[5])).take(1)
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 1; "take 1 returns only chain_top")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: vec![(agent_hash(&[0]), forked_chain(&[5..9, 0..7]))],
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(7)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "forked actions within 1 db, excludes forked actions from chain without chain top")]
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
agent_hash(&[0]), ChainFilter::new(action_hash(&[5])).take(5)
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
agent_hash(&[0]), ChainFilter::new(action_hash(&[5])).take(5)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "duplicates between db and scratch")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(2).until_hash(action_hash(&[3]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "take and until hash, take is higher seq")]
#[test_case(
FixtureDataStores {
    dht: FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(5).until_hash(action_hash(&[5]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "take and until hash, until hash is higher seq")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(7).until_timestamp(Timestamp::from_micros(5_i64 * 1000))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "take and until timestamp, until timestamp is higher seq")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(2).until_hash(action_hash(&[4])).until_timestamp(Timestamp::from_micros(3_i64 * 1000))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "all 3 filters, take is highest")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(7).until_hash(action_hash(&[5])).until_timestamp(Timestamp::from_micros(3_i64 * 1000))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "all 3 filters,, until_hash is highest")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(7).until_hash(action_hash(&[4])).until_timestamp(Timestamp::from_micros(5_i64 * 1000))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "all 3 filters, until_timestamp is highest")]
#[test_case(
    FixtureDataStores {
        dht:  FixtureData {
            activity: agent_chain(&[(0, 0..7)]),
            ..Default::default()
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).until_timestamp(Timestamp::from_micros(0))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "until_timestamp before earliest action timestamp")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(10)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "take is greater than total len, take full len")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).until_hash(action_hash(&[15]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "until_hash is not found, take full len")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).until_hash(action_hash(&[5])).until_hash(action_hash(&[4])).until_hash(action_hash(&[3]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "multiple until_hashes, highest seq is used")]
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
agent_hash(&[0]), ChainFilter::new(action_hash(&[7])).take(3)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 3; "take limit re-applied after merge from multiple dbs")]
#[test_case(
FixtureDataStores {
    dht:  FixtureData {
            activity: agent_chain(&[(0, 0..10)]),
                    ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[3])).until_hash(action_hash(&[5])).until_hash(action_hash(&[99]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "multiple until_hashes with one not found uses max of found")]
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
agent_hash(&[0]), ChainFilter::new(action_hash(&[12])).take(5)
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
agent_hash(&[0]), ChainFilter::new(action_hash(&[9])).take(20)
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
agent_hash(&[0]), ChainFilter::new(action_hash(&[11])).take(6).until_hash(action_hash(&[4]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 6; "activity from all stores with overlaps")]
#[test_case(
    FixtureDataStores {
        dht: FixtureData {
            activity: agent_chain(&[(0, 0..10)]),
            warrants: vec![warrant(0), warrant(0)],
        },
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[5])).take(3)
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
    agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[5]))
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
    agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).until_hash(action_hash(&[5]))
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
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with dht 0 till 2 with 1 unrelated chain warrant")]
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
async fn test_must_get_agent_activity_ok(
    data: FixtureDataStores,
    author: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_forks_split_between_2_dbs() {
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
    let filter = ChainFilter::new(action_hash(&[5])).take(5);

    let res = test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap();

    assert!(matches!(res, MustGetAgentActivityResponse::IncompleteChain))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_forks_split_between_db_and_scratch() {
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
    let filter = ChainFilter::new(action_hash(&[5])).take(5);

    let res = test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap();

    assert!(matches!(res, MustGetAgentActivityResponse::IncompleteChain))
}

#[test_case(
FixtureDataStores {
    dht: FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[3])).until_hash(action_hash(&[6]))
=> matches CascadeError::QueryError(StateQueryError::InvalidInput(_)); "until hash greater than chain top")]
#[test_case(
FixtureDataStores {
    dht: FixtureData {
        activity: agent_chain(&[(0, 0..7)]),
        ..Default::default()
    },
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).take(0)
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

#[tokio::test(flavor = "multi_thread")]
async fn test_deterministically_retain_forked_action_with_max_hash() {
    holochain_trace::test_run();

    // Create two forked actions with action_seq 3
    // The action from fork chain 1 has the higher hash value, so should be retained
    let author = agent_hash(&[0]);
    let chain = vec![(author.clone(), forked_chain(&[0..5, 3..6]))];

    let data = FixtureDataStores {
        dht: FixtureData {
            activity: chain,
            ..Default::default()
        },
        ..Default::default()
    };

    // Run multiple times to demonstrate it is deterministic
    for _ in 0..5 {
        // Run must_get_agent_activity using chain head of fork chain 1
        let res = test_must_get_agent_activity_inner(
            data.clone(),
            author.clone(),
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

        // The activity should be from fork chain 1
        assert_eq!(
            activity_with_action_seq_3.action.hashed.hash,
            TestChainHash::forked(3, 1).into(),
            "Expected fork with max hash to be retained"
        );
    }
}

// Demonstrate that a ChainFilter::UntilHash is actually used to retain actions with the action seq range from
// the chain top Action's action_seq to the UntilHash Action's action_seq.
//
// So if the UntilHash Action is on a fork chain that is *not* retained,
// we will still return the Action's on the retained chain until that same action seq.
#[tokio::test(flavor = "multi_thread")]
async fn test_chain_filter_until_hash_is_converted_to_until_action_seq() {
    holochain_trace::test_run();

    // Create fork at seq 3
    // Fork 0: seq 0,1,2,3,4 (will be excluded because lower hash)
    // Fork 1: seq 3,4,5,6,7 (will be retained because higher hash)
    let author = agent_hash(&[0]);
    let chain = vec![(author.clone(), forked_chain(&[0..5, 3..8]))];

    let data = FixtureDataStores {
        dht: FixtureData {
            activity: chain,
            ..Default::default()
        },
        ..Default::default()
    };

    // Run must_get_agent_activity with an UntilHash for an Action that is part of the *dropped* fork chain.
    let until_hash_dropped: ActionHash = TestChainHash::forked(4, 0).into();
    let chain_top = TestChainHash::forked(7, 1).into();
    let res = test_must_get_agent_activity_inner(
        data,
        author,
        ChainFilter::new(chain_top).until_hash(until_hash_dropped.clone()),
    )
    .await
    .unwrap();

    // We still receive activity from the chain top action seq to the until hash Action's action_seq,
    // but only with Actions from the retained fork chain.
    let activity = match res {
        MustGetAgentActivityResponse::Activity { activity, .. } => activity,
        other => panic!("Expected Activity response, got: {other:?}"),
    };
    assert!(activity.len() <= 8); // At most all actions
    assert!(activity.iter().any(|a| a.action.seq() == 4));

    // The UntilHash is not included in the results
    assert!(!activity
        .iter()
        .any(|a| a.action.hashed.hash == until_hash_dropped));
}
