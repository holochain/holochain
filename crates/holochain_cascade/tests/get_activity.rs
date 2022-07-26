use std::sync::Arc;

use ghost_actor::dependencies::observability;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_cascade::test_utils::*;
use holochain_cascade::Cascade;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_state::prelude::test_cache_db;
use holochain_state::prelude::test_dht_db;
use holochain_state::prelude::test_in_mem_db;
use holochain_types::activity::*;
use holochain_types::chain::MustGetAgentActivityResponse;
use holochain_types::test_utils::chain::*;
use holochain_zome_types::ChainFilter;
use holochain_zome_types::ChainStatus;
use test_case::test_case;

#[tokio::test(flavor = "multi_thread")]
async fn get_activity() {
    observability::test_run().ok();

    // Environments
    let cache = test_cache_db();
    let authority = test_dht_db();

    // Data
    let td = ActivityTestData::valid_chain_scenario();

    for hash_op in td.hash_ops.iter().cloned() {
        fill_db(&authority.to_db(), hash_op);
    }
    for hash_op in td.noise_ops.iter().cloned() {
        fill_db(&authority.to_db(), hash_op);
    }
    for hash_op in td.store_ops.iter().cloned() {
        fill_db(&cache.to_db(), hash_op);
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
    let mut cascade = Cascade::empty().with_network(network, cache.to_db());

    let r = cascade
        .get_agent_activity(td.agent.clone(), td.query_filter.clone(), options)
        .await
        .unwrap();

    let expected = AgentActivityResponse {
        agent: td.agent.clone(),
        valid_activity: td.valid_records.clone(),
        rejected_activity: ChainItems::NotRequested,
        status: ChainStatus::Valid(td.chain_head.clone()),
        highest_observed: Some(td.highest_observed.clone()),
    };
    assert_eq!(r, expected);
}

#[test_case(
    agent_chain(&[(0, 0..3), (0, 5..10)]), agent_hash(&[0]), ChainFilter::new(action_hash(&[8]))
    => MustGetAgentActivityResponse::IncompleteChain ; "8 to genesis with 0 till 2 and 5 till 9")]
#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity(
    chain: Vec<(AgentPubKey, Vec<ChainItem>)>,
    author: AgentPubKey,
    filter: ChainFilter,
) -> MustGetAgentActivityResponse {
    let authority = commit_chain(
        DbKindDht(Arc::new(DnaHash::from_raw_36(vec![0; 36]))),
        chain,
    );
    let cache = test_in_mem_db(DbKindCache(Arc::new(DnaHash::from_raw_36(vec![0; 36]))));
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.into()]);
    let mut cascade = Cascade::empty().with_network(network, cache);
    cascade
        .must_get_agent_activity(author, filter)
        .await
        .unwrap()
}
