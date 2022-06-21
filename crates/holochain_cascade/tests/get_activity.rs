use ghost_actor::dependencies::observability;
use holochain_cascade::test_utils::*;
use holochain_cascade::Cascade;
use holochain_state::prelude::test_cache_db;
use holochain_state::prelude::test_dht_db;
use holochain_types::activity::*;
use holochain_zome_types::ChainStatus;
use pretty_assertions::assert_eq;

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
        valid_activity: td.valid_commits.clone(),
        rejected_activity: ChainItems::NotRequested,
        status: ChainStatus::Valid(td.chain_head.clone()),
        highest_observed: Some(td.highest_observed.clone()),
    };
    assert_eq!(r, expected);
}
