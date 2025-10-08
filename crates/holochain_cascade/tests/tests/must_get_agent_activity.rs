use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_cascade::test_utils::*;
use holochain_cascade::CascadeImpl;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_state::integrate::authored_ops_to_dht_db_without_check;
use holochain_state::prelude::*;
use holochain_types::test_utils::chain::*;
use std::sync::Arc;
use test_case::test_case;
#[cfg(feature = "unstable-warrants")]
use {
    holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator},
    holochain_state::integrate::insert_locally_validated_op,
};

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

#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity(
    data: Data,
    author: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    test_must_get_agent_activity_inner(data, author, filter).await
}

#[cfg(feature = "unstable-warrants")]
#[test_case(
    Data { dht: agent_chain(&[(0, 0..3)]), warrants: vec![warrant(0)], ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with dht 0 till 2 with 1 unrelated chain warrant")]
#[test_case(
    Data { dht: agent_chain(&[(0, 0..3)]), warrants: vec![warrant(0)], ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { warrants, .. } if warrants.len() == 1; "1 to genesis with dht 0 till 2 with 1 chain warrant")]
#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_with_warrants(
    data: Data,
    author: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    test_must_get_agent_activity_inner(data, author, filter).await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_1() {
    let res = test_must_get_agent_activity_inner(
        Data { 
            dht: agent_chain(&[(0, 0..3)]), ..Default::default() 
        },
        agent_hash(&[0]), 
        ChainFilter::new(action_hash(&[1]))
    )
    .await;

    assert!(matches!(res, MustGetAgentActivityResponse::Activity {..}));
}

async fn test_must_get_agent_activity_inner(
    data: Data,
    author: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    tracing::error!("test_must_get_agent_activity_inner 1");

    let Data {
        scratch,
        authored,
        cache,
        dht,
        warrants,
    } = data;
    let dht = commit_chain(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))), dht);
    let network = PassThroughNetwork::authority_for_nothing(vec![dht.clone().into()]);
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
        .with_network(network, cache);
    if let Some(sync_scratch) = sync_scratch {
        cascade = cascade.with_scratch(sync_scratch);
    }

    cascade
        .must_get_agent_activity(author, filter)
        .await
        .unwrap()
}
