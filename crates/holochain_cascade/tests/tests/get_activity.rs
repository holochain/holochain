use std::sync::Arc;

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
use test_case::test_case;

#[tokio::test(flavor = "multi_thread")]
async fn get_activity() {
    holochain_trace::test_run();

    // Environments
    let cache = test_cache_db();
    let authority = test_dht_db();

    // Data
    let td = ActivityTestData::valid_chain_scenario();

    for hash_op in td.hash_ops.iter().cloned() {
        fill_db(&authority.to_db(), hash_op).await;
    }
    for hash_op in td.noise_ops.iter().cloned() {
        fill_db(&authority.to_db(), hash_op).await;
    }
    for hash_op in td.store_ops.iter().cloned() {
        fill_db(&cache.to_db(), hash_op).await;
    }

    let options = holochain_p2p::actor::GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_actions: true,
        ..Default::default()
    };

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.to_db().clone().into()]);

    // Cascade
    let cascade = CascadeImpl::empty().with_network(network, cache.to_db());

    let r = cascade
        .get_agent_activity(td.agent.clone(), ChainQueryFilter::new(), options)
        .await
        .unwrap();

    let expected = AgentActivityResponse {
        agent: td.agent.clone(),
        valid_activity: td.valid_records.clone(),
        rejected_activity: ChainItems::NotRequested,
        warrants: vec![],
        status: ChainStatus::Valid(td.chain_head.clone()),
        highest_observed: Some(td.highest_observed.clone()),
    };
    assert_eq!(r, expected);
}

// #[tokio::test(flavor = "multi_thread")]
// async fn test_must_get_agent_activity_with_warrants() {
//     let dht = commit_chain(
//         DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
//         agent_chain(&[(0, 0..3)]),
//     );
//     let cache = commit_chain(
//         DbKindCache(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
//         vec![],
//     );
//     let authored = commit_chain(
//         DbKindAuthored(Arc::new(CellId::new(
//             DnaHash::from_raw_36(vec![0; 36]),
//             AgentPubKey::from_raw_36(vec![0; 36]),
//         ))),
//         vec![],
//     );
//     let network = PassThroughNetwork::authority_for_nothing(vec![dht.into()]);
//     let cascade = CascadeImpl::empty()
//         .with_authored(authored.into())
//         .with_network(network, cache);
//     let filter = ChainFilter::new(action_hash(&[2]));
//     let r = cascade
//         .must_get_agent_activity(agent_hash(&[0]), filter)
//         .await
//         .unwrap();
//     dbg!(&r);
// }

#[derive(Default)]
struct Data {
    scratch: Option<Vec<(AgentPubKey, Vec<TestChainItem>)>>,
    authored: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    cache: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    dht: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    warrants: Vec<WarrantOp>,
}

fn warrant(author: u8, action: u8) -> WarrantOp {
    let p = WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
        action_author: AgentPubKey::from_raw_36(vec![author; 36]),
        action: (
            ActionHash::from_raw_36(vec![action; 36]),
            ::fixt::fixt!(Signature),
        ),
        validation_type: ValidationType::Sys,
    });
    let warrant = Warrant::new(p, AgentPubKey::from_raw_36(vec![255; 36]), Timestamp::now());
    WarrantOp::from(SignedWarrant::new(warrant, ::fixt::fixt!(Signature)))
}

#[test_case(
    Data { dht: agent_chain(&[(0, 0..3)]), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity(a) if a.len() == 2; "1 to genesis with dht 0 till 2")]
#[test_case(
    Data { dht: agent_chain(&[(0, 0..3)]), warrants: vec![warrant(1, 1)], ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity(a) if a.len() == 2; "1 to genesis with dht 0 till 2 with 1 unrelated chain warrant")]
#[test_case(
    Data { dht: agent_chain(&[(0, 0..3)]), warrants: vec![warrant(0, 0)], ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Warrants(a) if a.len() == 1; "1 to genesis with dht 0 till 2 with 1 chain warrant")]
#[test_case(
    Data { cache: agent_chain(&[(0, 0..3)]), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity(a) if a.len() == 2; "1 to genesis with cache 0 till 2")]
#[test_case(
    Data { scratch: Some(agent_chain(&[(0, 0..3)])), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity(a) if a.len() == 2; "1 to genesis with scratch 0 till 2")]
#[test_case(
    Data { authored: agent_chain(&[(0, 0..3)]), scratch: Some(agent_chain(&[(0, 3..6)])), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until(action_hash(&[0]))
    => matches MustGetAgentActivityResponse::Activity(a) if a.len() == 4; "4 take 4 until 0 with authored 0 till 2 and scratch 3 till 5")]
#[test_case(
    Data { authored: agent_chain(&[(0, 0..6)]), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until(action_hash(&[0]))
    => matches MustGetAgentActivityResponse::Activity(a) if a.len() == 4; "4 take 4 until 0 with authored 0 till 5")]
#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity(
    data: Data,
    author: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
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
    authored.test_write(|txn| {
        for w in warrants {
            let w = DhtOp::from(w).into_hashed();
            insert_op(txn, &w).unwrap();
        }
    });

    let dht_cache = DhtDbQueryCache::new(dht.clone().into());
    authored_ops_to_dht_db_without_check(hashes, authored.clone().into(), dht, &dht_cache)
        .await
        .unwrap();

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
