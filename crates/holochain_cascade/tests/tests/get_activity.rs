use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_cascade::error::CascadeResult;
use holochain_cascade::test_utils::*;
use holochain_cascade::CascadeImpl;
use holochain_p2p::actor::GetActivityOptions;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_state::integrate::authored_ops_to_dht_db_without_check;
use holochain_state::prelude::*;
use holochain_types::test_utils::chain::*;
use std::sync::Arc;
use test_case::test_case;

macro_rules! assert_agent_activity_responses_eq {
    ($expected:expr, $actual:expr) => {
        assert_eq!($expected.agent, $actual.agent);
        match (&$expected.valid_activity, &$actual.valid_activity) {
            (ChainItems::Full(e), ChainItems::Full(a)) => {
                assert_eq!(e.len(), a.len());
                for (i, (e, a)) in e.iter().zip(a.iter()).enumerate() {
                    assert_eq!(e, a, "Not equal at index {}", i);
                }
            }
            (ChainItems::Hashes(e), ChainItems::Hashes(a)) => {
                assert_eq!(e.len(), a.len());
                for (i, (e, a)) in e.iter().zip(a.iter()).enumerate() {
                    assert_eq!(e, a, "Not equal at index {}", i);
                }
            }
            (ChainItems::NotRequested, ChainItems::NotRequested) => {}
            (l, r) => panic!("Not equal: {:?} != {:?}", l, r),
        }
        assert_eq!($expected.valid_activity, $actual.valid_activity);
        assert_eq!($expected.rejected_activity, $actual.rejected_activity);
        assert_eq!($expected.warrants, $actual.warrants);
        assert_eq!($expected.status, $actual.status);
        assert_eq!($expected.highest_observed, $actual.highest_observed);
    };
}

/// A simple test that we can get the activity for an agent
#[tokio::test(flavor = "multi_thread")]
async fn get_activity() {
    holochain_trace::test_run();

    let test_data = ActivityTestData::valid_chain_scenario(false);

    let scenario = GetActivityTestScenario::new(test_data.clone())
        .include_agent_activity_ops_in_dht_db()
        .await
        .include_store_entry_ops_in_dht_db()
        .await
        .include_agent_activity_noise_ops_in_dht_db()
        .await
        .include_store_entry_ops_in_cache_db()
        .await;

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        ..Default::default()
    };

    let r = scenario.query_authority(options).await.unwrap();

    let expected = AgentActivityResponse {
        agent: test_data.agent.clone(),
        valid_activity: test_data.valid_hashes.clone(),
        rejected_activity: ChainItems::NotRequested,
        warrants: vec![],
        status: ChainStatus::Valid(test_data.chain_head.clone()),
        highest_observed: Some(test_data.highest_observed.clone()),
    };

    assert_agent_activity_responses_eq!(expected, r);
}

/// Check that the different options for getting chain items will return the same records
#[tokio::test(flavor = "multi_thread")]
async fn get_activity_chain_items_parity() {
    holochain_trace::test_run();

    let test_data = ActivityTestData::valid_chain_scenario(false);

    let scenario = GetActivityTestScenario::new(test_data.clone())
        .include_agent_activity_ops_in_dht_db()
        .await
        .include_store_entry_ops_in_dht_db()
        .await
        .include_agent_activity_noise_ops_in_dht_db()
        .await
        .with_chain_filter(ChainQueryFilter::new().include_entries(true));

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        ..Default::default()
    };

    let with_hashes = scenario.query_authority(options.clone()).await.unwrap();

    assert_agent_activity_responses_eq!(
        AgentActivityResponse {
            agent: test_data.agent.clone(),
            valid_activity: test_data.valid_hashes.clone(),
            rejected_activity: ChainItems::NotRequested,
            warrants: vec![],
            status: ChainStatus::Valid(test_data.chain_head.clone()),
            highest_observed: Some(test_data.highest_observed.clone()),
        },
        with_hashes
    );

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: true,
        ..Default::default()
    };

    let with_records = scenario.query_authority(options.clone()).await.unwrap();

    assert_agent_activity_responses_eq!(
        AgentActivityResponse {
            agent: test_data.agent.clone(),
            valid_activity: test_data.valid_records.clone(),
            rejected_activity: ChainItems::NotRequested,
            warrants: vec![],
            status: ChainStatus::Valid(test_data.chain_head.clone()),
            highest_observed: Some(test_data.highest_observed.clone()),
        },
        with_records
    );

    let hashes_from_hashes: Vec<ActionHash> = match with_hashes.valid_activity {
        ChainItems::Hashes(h) => h.into_iter().map(|(_, h)| h).collect(),
        _ => unreachable!(),
    };
    let hashes_from_records: Vec<ActionHash> = match with_records.valid_activity {
        ChainItems::Full(r) => r.into_iter().map(|r| r.action_hash().clone()).collect(),
        _ => unreachable!(),
    };

    assert_eq!(hashes_from_hashes, hashes_from_records);
}

/// When an AAA doesn't have the entries for the requested records, then the cascade should try to
/// retrieve the entries from either the cache or the network. In this test we check that it's
/// possible to retrieve entries from the cache.
#[tokio::test(flavor = "multi_thread")]
async fn fill_records_entries() {
    holochain_trace::test_run();

    let test_data = ActivityTestData::valid_chain_scenario(false);

    let scenario = GetActivityTestScenario::new(test_data.clone())
        .include_agent_activity_ops_in_dht_db()
        .await
        .include_agent_activity_noise_ops_in_dht_db()
        .await
        .include_store_entry_ops_in_cache_db()
        .await
        .with_chain_filter(ChainQueryFilter::new().include_entries(true));

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: true,
        get_options: GetOptions::local(),
        ..Default::default()
    };

    let r = scenario.query_authority(options).await.unwrap();

    let expected = AgentActivityResponse {
        agent: test_data.agent.clone(),
        valid_activity: test_data.valid_records.clone(),
        rejected_activity: ChainItems::NotRequested,
        warrants: vec![],
        status: ChainStatus::Valid(test_data.chain_head.clone()),
        highest_observed: Some(test_data.highest_observed.clone()),
    };
    assert_agent_activity_responses_eq!(expected, r);
}

/// Try fetching locally and remotely, and forced remotely to check that all routes return the same
/// result
#[tokio::test(flavor = "multi_thread")]
async fn fetch_routes_parity() {
    holochain_trace::test_run();

    let test_data = ActivityTestData::valid_chain_scenario(false);

    let scenario = GetActivityTestScenario::new(test_data.clone())
        .include_agent_activity_ops_in_dht_db()
        .await
        .include_store_entry_ops_in_dht_db()
        .await
        .include_agent_activity_noise_ops_in_dht_db()
        .await
        .include_store_entry_ops_in_cache_db()
        .await;

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: true,
        get_options: GetOptions::local(),
        ..Default::default()
    };

    // Note that we're actually sharing one dht+cache database here so the local and remote
    // aren't quite accurate here. But the point is to exercise the code paths and not to test
    // that local and remote data are actually merged correctly.

    let local_with_remote_authority = scenario.query_authority(options.clone()).await.unwrap();

    let local_as_self_authority = scenario.query_self(options.clone()).await.unwrap();

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: true,
        get_options: GetOptions::network(),
        ..Default::default()
    };

    let remote_with_remote_authority = scenario.query_authority(options.clone()).await.unwrap();

    let remote_as_self_authority = scenario.query_self(options.clone()).await.unwrap();

    assert_agent_activity_responses_eq!(local_with_remote_authority, local_as_self_authority);
    assert_agent_activity_responses_eq!(local_with_remote_authority, remote_with_remote_authority);
    assert_agent_activity_responses_eq!(local_with_remote_authority, remote_as_self_authority);
}

/// Check that getting activity with records will not serve private entries
#[tokio::test(flavor = "multi_thread")]
async fn record_activity_does_not_serve_private_entries() {
    holochain_trace::test_run();

    let mut test_data = ActivityTestData::valid_chain_scenario(true);

    // Wipe out the private entries in the expected response
    match &mut test_data.valid_records {
        ChainItems::Full(records) => {
            // Skip 1 to leave the DNA entry as `NA` rather than `Hidden`
            records.iter_mut().skip(1).for_each(|r| {
                r.entry = RecordEntry::Hidden;
            })
        }
        _ => unreachable!(),
    }

    let scenario = GetActivityTestScenario::new(test_data.clone())
        .include_agent_activity_ops_in_dht_db()
        .await
        .include_store_entry_ops_in_dht_db()
        .await
        .include_agent_activity_noise_ops_in_dht_db()
        .await;

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: true,
        get_options: GetOptions::local(),
        ..Default::default()
    };

    let r = scenario.query_authority(options).await.unwrap();

    let expected = AgentActivityResponse {
        agent: test_data.agent.clone(),
        valid_activity: test_data.valid_records.clone(),
        rejected_activity: ChainItems::NotRequested,
        warrants: vec![],
        status: ChainStatus::Valid(test_data.chain_head.clone()),
        highest_observed: Some(test_data.highest_observed.clone()),
    };

    assert_agent_activity_responses_eq!(expected, r);
}

/// Checks that actions can be requested with a chain query filter that drops entries
#[tokio::test(flavor = "multi_thread")]
async fn filter_out_entries_with_chain_query() {
    holochain_trace::test_run();

    let mut test_data = ActivityTestData::valid_chain_scenario(false);

    // Wipe out the entries in the expected response
    match &mut test_data.valid_records {
        ChainItems::Full(records) => {
            // Skip 1 to leave the DNA entry as `NA` rather than `NotStored`
            records.iter_mut().skip(1).for_each(|r| {
                r.entry = RecordEntry::NotStored;
            })
        }
        _ => unreachable!(),
    }

    let filter = ChainQueryFilter::new().include_entries(false);

    let scenario = GetActivityTestScenario::new(test_data.clone())
        .with_chain_filter(filter)
        .include_agent_activity_ops_in_dht_db()
        .await
        .include_store_entry_ops_in_dht_db()
        .await
        .include_agent_activity_noise_ops_in_dht_db()
        .await;

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: true,
        get_options: GetOptions::local(),
        ..Default::default()
    };

    let r = scenario.query_authority(options).await.unwrap();

    let expected = AgentActivityResponse {
        agent: test_data.agent.clone(),
        valid_activity: test_data.valid_records.clone(),
        rejected_activity: ChainItems::NotRequested,
        warrants: vec![],
        status: ChainStatus::Valid(test_data.chain_head.clone()),
        highest_observed: Some(test_data.highest_observed.clone()),
    };

    assert_agent_activity_responses_eq!(expected, r);
}

#[cfg(feature = "unstable-warrants")]
#[tokio::test(flavor = "multi_thread")]
async fn get_activity_with_warrants() {
    holochain_trace::test_run();

    // DBs
    let cache = test_cache_db();
    let dht = test_dht_db();

    // Data
    let td = ActivityTestData::valid_chain_scenario(false);

    for hash_op in td.agent_activity_ops.iter().cloned() {
        fill_db(&dht.to_db(), hash_op).await;
    }
    for hash_op in td.noise_agent_activity_ops.iter().cloned() {
        fill_db(&dht.to_db(), hash_op).await;
    }
    for hash_op in td.store_entry_ops.iter().cloned() {
        fill_db(&cache.to_db(), hash_op).await;
    }

    let warrant = {
        let action_pair = (
            (
                td.agent_activity_ops[0].action().to_hash(),
                ::fixt::fixt!(Signature),
            ),
            (
                td.agent_activity_ops[1].action().to_hash(),
                ::fixt::fixt!(Signature),
            ),
        );
        let p = WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
            chain_author: td.agent.clone(),
            action_pair,
        });
        let warrant = Warrant::new(p, AgentPubKey::from_raw_36(vec![255; 36]), Timestamp::now());
        WarrantOp::from(SignedWarrant::new(warrant, ::fixt::fixt!(Signature)))
    };

    // Insert unvalidated warrant op
    dht.test_write({
        let op = DhtOp::from(warrant.clone()).into_hashed();
        move |txn| {
            insert_op_dht(txn, &op, None).unwrap();
        }
    });

    let options = GetActivityOptions {
        include_valid_activity: true,
        include_rejected_activity: false,
        include_full_records: true,
        ..Default::default()
    };

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![dht.to_db().clone().into()]);

    // Cascade
    let cascade = CascadeImpl::empty().with_network(network, cache.to_db());

    let mut expected = AgentActivityResponse {
        agent: td.agent.clone(),
        valid_activity: td.valid_records.clone(),
        rejected_activity: ChainItems::NotRequested,
        warrants: vec![],
        status: ChainStatus::Valid(td.chain_head.clone()),
        highest_observed: Some(td.highest_observed.clone()),
    };

    let r1 = cascade
        .get_agent_activity(
            td.agent.clone(),
            ChainQueryFilter::new().include_entries(true),
            options.clone(),
        )
        .await
        .unwrap();

    assert_agent_activity_responses_eq!(expected, r1);

    // If the warrant is validated, it will be returned
    dht.test_write({
        let op = DhtOp::from(warrant.clone()).into_hashed();
        let op_hash = op.as_hash().clone();
        move |txn| {
            holochain_state::mutations::set_validation_status(
                txn,
                &op_hash,
                ValidationStatus::Valid,
            )
            .unwrap();
            holochain_state::mutations::set_when_integrated(txn, &op_hash, Timestamp::now())
                .unwrap();
        }
    });

    let r2 = cascade
        .get_agent_activity(
            td.agent.clone(),
            ChainQueryFilter::new().include_entries(true),
            options,
        )
        .await
        .unwrap();

    expected.warrants = vec![warrant.into_warrant()];

    assert_eq!(r2, expected);
}

#[derive(Default)]
struct Data {
    scratch: Option<Vec<(AgentPubKey, Vec<TestChainItem>)>>,
    authored: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    cache: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    dht: Vec<(AgentPubKey, Vec<TestChainItem>)>,
    warrants: Vec<WarrantOp>,
}

#[cfg(feature = "unstable-warrants")]
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

struct GetActivityTestScenario {
    dht: TestDb<DbKindDht>,
    cache: TestDb<DbKindCache>,
    test_data: ActivityTestData,
    chain_filter: ChainQueryFilter,
}

impl GetActivityTestScenario {
    fn new(test_data: ActivityTestData) -> Self {
        let dht = test_dht_db();
        let cache = test_cache_db();

        Self {
            dht,
            cache,
            test_data,
            chain_filter: ChainQueryFilter::new(),
        }
    }

    async fn include_agent_activity_ops_in_dht_db(self) -> Self {
        for hash_op in self.test_data.agent_activity_ops.iter().cloned() {
            fill_db(&self.dht.to_db(), hash_op).await;
        }

        self
    }

    async fn include_agent_activity_noise_ops_in_dht_db(self) -> Self {
        for hash_op in self.test_data.noise_agent_activity_ops.iter().cloned() {
            fill_db(&self.dht.to_db(), hash_op).await;
        }

        self
    }

    async fn include_store_entry_ops_in_dht_db(self) -> Self {
        for hash_op in self.test_data.store_entry_ops.iter().cloned() {
            fill_db(&self.dht.to_db(), hash_op).await;
        }

        self
    }

    async fn include_store_entry_ops_in_cache_db(self) -> Self {
        for hash_op in self.test_data.store_entry_ops.iter().cloned() {
            fill_db(&self.cache.to_db(), hash_op).await;
        }

        self
    }

    fn with_chain_filter(mut self, chain_filter: ChainQueryFilter) -> Self {
        self.chain_filter = chain_filter;
        self
    }

    async fn query_authority(
        &self,
        options: GetActivityOptions,
    ) -> CascadeResult<AgentActivityResponse> {
        let network =
            PassThroughNetwork::authority_for_nothing(vec![self.dht.to_db().clone().into()]);

        let cascade = CascadeImpl::empty().with_network(network, self.cache.to_db());

        self.query(cascade, options).await
    }

    async fn query_self(
        &self,
        options: GetActivityOptions,
    ) -> CascadeResult<AgentActivityResponse> {
        let network = PassThroughNetwork::authority_for_all(vec![self.dht.to_db().clone().into()]);

        let cascade = CascadeImpl::empty()
            .with_network(network, self.cache.to_db())
            .with_dht(self.dht.clone().into());

        self.query(cascade, options).await
    }

    async fn query(
        &self,
        cascade: CascadeImpl,
        options: GetActivityOptions,
    ) -> CascadeResult<AgentActivityResponse> {
        cascade
            .get_agent_activity(
                self.test_data.agent.clone(),
                self.chain_filter.clone(),
                options,
            )
            .await
    }
}

#[test_case(
    Data { dht: agent_chain(&[(0, 0..3)]), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with dht 0 till 2")]
#[test_case(
    Data { cache: agent_chain(&[(0, 0..3)]), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with cache 0 till 2")]
#[test_case(
    Data { scratch: Some(agent_chain(&[(0, 0..3)])), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with scratch 0 till 2")]
#[test_case(
    Data { authored: agent_chain(&[(0, 0..3)]), scratch: Some(agent_chain(&[(0, 3..6)])), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until(action_hash(&[0]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "4 take 4 until 0 with authored 0 till 2 and scratch 3 till 5")]
#[test_case(
    Data { authored: agent_chain(&[(0, 0..6)]), ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[4])).take(4).until(action_hash(&[0]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 4; "4 take 4 until 0 with authored 0 till 5")]
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
    Data { dht: agent_chain(&[(0, 0..3)]), warrants: vec![warrant(1, 1)], ..Default::default() },
    agent_hash(&[0]), ChainFilter::new(action_hash(&[1]))
    => matches MustGetAgentActivityResponse::Activity { activity, .. } if activity.len() == 2; "1 to genesis with dht 0 till 2 with 1 unrelated chain warrant")]
#[test_case(
    Data { dht: agent_chain(&[(0, 0..3)]), warrants: vec![warrant(0, 0)], ..Default::default() },
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

async fn test_must_get_agent_activity_inner(
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
            insert_op_authored(txn, &w).unwrap();
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
