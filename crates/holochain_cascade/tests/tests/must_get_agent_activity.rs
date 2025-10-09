use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_cascade::error::CascadeError;
use holochain_cascade::error::CascadeResult;
use holochain_cascade::test_utils::*;
use holochain_cascade::CascadeImpl;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_state::integrate::authored_ops_to_dht_db_without_check;
use holochain_state::prelude::*;
use holochain_state::query::StateQueryError;
use holochain_types::test_utils::chain::*;
use std::sync::Arc;
use test_case::test_case;
#[cfg(feature = "unstable-warrants")]
use {
    holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator},
    holochain_state::integrate::insert_locally_validated_op,
};

#[cfg(feature = "unstable-warrants")]
fn warrant(warrantee: u8) -> WarrantOp {
    let p = WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
        action_author: ::fixt::fixt!(AgentPubKey),
        action: (::fixt::fixt!(ActionHash), ::fixt::fixt!(Signature)),
        chain_op_type: ChainOpType::StoreRecord,
    });
    let warrant = Warrant::new(
        p,
        ::fixt::fixt!(AgentPubKey),
        Timestamp::now(),
        AgentPubKey::from_raw_36(vec![warrantee; 36]),
    );
    WarrantOp::from(SignedWarrant::new(warrant, ::fixt::fixt!(Signature)))
}

#[derive(Default, Debug)]
struct Data {
    scratch: Option<Vec<(AgentPubKey, Vec<TestChainItem>)>>,
    authored: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    cache: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    dht: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    warrants: Vec<WarrantOp>,
}

#[test_case(
    Data {
        dht: agent_chain(&[(0, 0..3)]), ..Default::default()
    },
    agent_hash(&[0]),
    ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with dht 0 till 2")]
#[test_case(
    Data {
        cache: agent_chain(&[(0, 0..3)]), ..Default::default()
    },
    agent_hash(&[0]),
    ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with cache 0 till 2")]
#[test_case(
    Data {
        authored: agent_chain(&[(0, 0..6)]), ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until_hash(action_hash(&[0]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "4 take 4 until 0 with authored 0 till 5")]
#[test_case(
    Data {
        scratch: Some(agent_chain(&[(0, 0..3)])), ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with scratch 0 till 2")]
#[test_case(
    Data {
        authored: agent_chain(&[(0, 0..3)]),
        scratch: Some(agent_chain(&[(0, 3..6)])), ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until_hash(action_hash(&[0]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "4 take 4 until 0 with authored 0 till 2 and scratch 3 till 5")]
#[test_case(
    Data {
        dht: agent_chain(&[(0, 0..3)]),
        cache: agent_chain(&[(0, 3..6)]),
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until_hash(action_hash(&[0]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "4 take 4 until 0 with dht 0 till 2 and cache 3 till 5")]
#[test_case(
    Data {
        dht: agent_chain(&[(0, 0..7)]),
        cache: agent_chain(&[(1, 1..4)]),
        ..Default::default()
    },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "4 to genesis with dht 0 till 7 and cache forked 1 till 4")]
#[test_case(
Data {
    authored: agent_chain(&[(0, 0..7)]),
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[4]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "4 to genesis with dht 0 till 7 and authored 0 till 7")]
#[test_case(
Data {
    authored: agent_chain(&[(0, 0..7)]),
    dht: agent_chain(&[(0, 0..7)]),
    scratch: Some(agent_chain(&[(0, 7..9)])),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).take(4)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "8 take 4 with dht 0 till 7 and authored 0 till 7 and scratch 7 till 9")]
#[test_case(
Data {
    dht: vec![(agent_hash(&[0]), forked_chain(&[5..9, 0..7]))],
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(7)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "forked actions within 1 db, excludes forked actions from chain without chain top")]
#[test_case(
Data {
    dht: vec![(agent_hash(&[0]), forked_chain(&[0..6, 3..8]))],
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(TestChainHash::forked(7, 1).into())
=> matches MustGetAgentActivityResponse::Activity { activity, ..} if activity.len() == 8; "forked actions within 1 db, chain top on retained forked actions")]
#[test_case(
Data {
    dht: vec![(agent_hash(&[0]), forked_chain(&[5..9, 0..7]))],
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(TestChainHash::forked(8, 0).into())
=> matches MustGetAgentActivityResponse::IncompleteChain; "forked actions within 1 db, chain top on excluded forked actions")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    cache: vec![(agent_hash(&[0]), forked_chain(&[0..3, ]))],
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[5])).take(5)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "forked actions between 2 dbs")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    scratch: Some(vec![(agent_hash(&[0]), forked_chain(&[0..3, ]))]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[5])).take(5)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "forked actions between db and scratch")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    cache: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[5])).take(5)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "duplicates between 2 dbs")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    scratch: Some(agent_chain(&[(0, 0..7)])),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[5])).take(5)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 5; "duplicates between db and scratch")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(2).until_hash(action_hash(&[3]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "take and until hash, take is higher seq")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(5).until_hash(action_hash(&[5]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "take and until hash, until hash is higher seq")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(7).until_timestamp(Timestamp::from_micros(5_i64 * 1000))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "take and until timestamp, until timestamp is higher seq")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(2).until_hash(action_hash(&[4])).until_timestamp(Timestamp::from_micros(3_i64 * 1000))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "take and until_hash and until_timestamp, take is highest seq")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(7).until_hash(action_hash(&[5])).until_timestamp(Timestamp::from_micros(3_i64 * 1000))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "take and until_hash and until_timestamp, until_hash is highest seq")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(7).until_hash(action_hash(&[4])).until_timestamp(Timestamp::from_micros(5_i64 * 1000))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "take and until_hash and until_timestamp, until_timestamp is highest seq")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).take(10)
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "take is greater than total len, take full len")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).until_hash(action_hash(&[15]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "until_hash is not found, take full len")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6])).until_hash(action_hash(&[5])).until_hash(action_hash(&[4])).until_hash(action_hash(&[3]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "multiple until_hashes, highest seq is used")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 5..7)]),
    cache: agent_chain(&[(0, 0..5)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "chain top in 1 db, additional activity below chain top in other")]
#[test_case(
Data {
    scratch: Some(agent_chain(&[(0, 5..7)])),
    cache: agent_chain(&[(0, 0..5)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[6]))
=> matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 7; "chain top in scratch, additional activity below chain top in other db")]
#[test_case(
Data {
    cache: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[8]))
=> matches MustGetAgentActivityResponse::ChainTopNotFound(_); "chain top not found")]
#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_ok(
    data: Data,
    author: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap()
}

#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[3])).until_hash(action_hash(&[6]))
=> matches CascadeError::QueryError(StateQueryError::InvalidInput(_)); "until hash greater than chain top")]
#[test_case(
Data {
    dht: agent_chain(&[(0, 0..7)]),
    ..Default::default()
},
agent_hash(&[0]), ChainFilter::new(action_hash(&[8])).take(0)
=> matches CascadeError::InvalidInput(_); "take 0 invalid input")]
#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_err(
    data: Data,
    author: AgentPubKey,
    filter: ChainFilter,
) -> CascadeError {
    test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap_err()
}

#[cfg(feature = "unstable-warrants")]
#[test_case(
    Data {dht: agent_chain(&[(0, 0..3)]), warrants: vec![warrant(0)], ..Default::default()},
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with dht 0 till 2 with 1 unrelated chain warrant")]
#[test_case(
    Data {dht: agent_chain(&[(0, 0..3)]), warrants: vec![warrant(0)], ..Default::default()},
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { warrants, .. } if warrants.len() == 1; "1 to genesis with dht 0 till 2 with 1 chain warrant")]
#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_with_warrants(
    data: Data,
    author: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    test_must_get_agent_activity_inner(data, author, filter)
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_1() {
    let res = test_must_get_agent_activity_inner(
        Data {
            dht: agent_chain(&[(0, 0..3)]),
            ..Default::default()
        },
        agent_hash(&[0]),
        ChainFilter::new(action_hash(&[1])),
    )
    .await;

    assert!(matches!(
        res,
        Ok(MustGetAgentActivityResponse::Activity { .. })
    ));
}

async fn test_must_get_agent_activity_inner(
    data: Data,
    author: AgentPubKey,
    filter: ChainFilter,
) -> CascadeResult<MustGetAgentActivityResponse> {
    let Data {
        scratch,
        authored,
        cache,
        dht,
        warrants,
    } = data;
    let dht = commit_chain(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))), dht);
    let cache = commit_chain(
        DbKindCache(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
        cache,
    );
    let authored = commit_chain(
        DbKindAuthored(Arc::new(CellId::new(
            DnaHash::from_raw_36(vec![0; 36]),
            AgentPubKey::from_raw_36(vec![0; 36]),
        ))),
        authored,
    );
    let hashes = warrants.iter().map(|w| w.to_hash()).collect();
    let authored_warrants = warrants.clone();
    authored.test_write(|txn| {
        for w in authored_warrants {
            let w = DhtOp::from(w).into_hashed();
            insert_op_authored(txn, &w).unwrap();
        }
    });

    authored_ops_to_dht_db_without_check(hashes, authored.clone().into(), dht.clone())
        .await
        .unwrap();
    #[cfg(feature = "unstable-warrants")]
    {
        dht.test_write(|txn| {
            for warrant in warrants {
                insert_locally_validated_op(txn, DhtOpHashed::from_content_sync(warrant)).unwrap();
            }
        });
    }

    let sync_scratch = match scratch {
        Some(scratch) => {
            let sync_scratch = Scratch::new().into_sync();
            commit_scratch(sync_scratch.clone(), scratch);
            Some(sync_scratch)
        }
        None => None,
    };
    let mut cascade = CascadeImpl::empty()
        .with_authored(authored.into())
        .with_cache(cache)
        .with_dht(dht.into());
    if let Some(sync_scratch) = sync_scratch {
        cascade = cascade.with_scratch(sync_scratch);
    }

    cascade.must_get_agent_activity(author, filter).await
}
