use fallible_iterator::FallibleIterator;
use hdk::prelude::Element;
use hdk::prelude::EntryType;
use hdk::prelude::ValidationPackage;
use holo_hash::HeaderHash;
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::HolochainP2pCellT;
use holochain_sqlite::db::DbRead;
use holochain_sqlite::fresh_reader_test;
use holochain_test_wasm_common::AgentActivitySearch;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use matches::assert_matches;

use crate::conductor::ConductorHandle;
use crate::test_utils::conductor_setup::CellHostFnCaller;
use crate::test_utils::conductor_setup::ConductorTestData;
use crate::test_utils::host_fn_caller::Post;
use crate::test_utils::new_zome_call;
use crate::test_utils::wait_for_integration;
use holochain_cascade::Cascade;
use holochain_cascade::DbPair;
use holochain_cascade::DbPairMut;
use holochain_state::element_buf::ElementBuf;
use holochain_state::metadata::ChainItemKey;
use holochain_state::metadata::MetadataBuf;
use holochain_state::metadata::MetadataBufT;
use holochain_state::source_chain::SourceChain;

const NUM_COMMITS: usize = 5;
const GET_AGENT_ACTIVITY_TIMEOUT_MS: u64 = 1000;
// Check if the correct number of ops are integrated
// every 100 ms for a maximum of 10 seconds but early exit
// if they are there.
const NUM_ATTEMPTS: usize = 100;
const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);

#[tokio::test(threaded_scheduler)]
async fn get_validation_package_test() {
    observability::test_run().ok();

    let zomes = vec![TestWasm::Create];
    let mut conductor_test = ConductorTestData::two_agents(zomes, false).await;
    let handle = conductor_test.handle();
    let alice_call_data = conductor_test.alice_call_data_mut();
    let alice_cell_id = &alice_call_data.cell_id;
    let alice_agent_id = alice_cell_id.agent_pubkey();

    // Helper to get header hashed
    let get_header = {
        let env = alice_call_data.env.clone();
        move |header_hash| {
            let alice_authored = ElementBuf::authored(env.clone().into(), false).unwrap();
            alice_authored
                .get_header(header_hash)
                .unwrap()
                .unwrap()
                .into_header_and_signature()
                .0
        }
    };

    let header_hash = commit_some_data("create_entry", &alice_call_data, &handle).await;

    // Expecting every header from the latest to the beginning
    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let alice_authored = alice_source_chain.elements();
    let expected_package = alice_source_chain
        .iter_back()
        // Skip the actual entry
        .skip(1)
        .filter_map(|shh| alice_authored.get_element(shh.header_address()))
        .collect::<Vec<_>>()
        .unwrap();

    let expected_package = Some(ValidationPackage::new(expected_package)).into();

    // Network call
    let validation_package = alice_call_data
        .network
        .get_validation_package(alice_agent_id.clone(), header_hash.clone())
        .await
        .unwrap();

    assert_eq!(validation_package, expected_package);

    // Cascade
    let header_hashed = get_header(&header_hash);
    let validation_package = check_cascade(&header_hashed, &alice_call_data).await;

    assert_eq!(validation_package, expected_package.0);

    // What happens if we commit a private entry?
    let header_hash_priv = commit_some_data("create_priv_msg", &alice_call_data, &handle).await;

    // Network
    // Check we still get the last package with new commits
    let validation_package = alice_call_data
        .network
        .get_validation_package(alice_agent_id.clone(), header_hash.clone())
        .await
        .unwrap();

    assert_eq!(validation_package, expected_package);

    // Cascade
    let header_hashed = get_header(&header_hash);
    let validation_package = check_cascade(&header_hashed, &alice_call_data).await;

    assert_eq!(validation_package, expected_package.0);

    // Get the package for the private entry, this is still full chain
    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let alice_authored = alice_source_chain.elements();
    let expected_package = alice_source_chain
        .iter_back()
        // Skip the actual entry
        .skip(1)
        .filter_map(|shh| alice_authored.get_element(shh.header_address()))
        .collect::<Vec<_>>()
        .unwrap();

    let expected_package = Some(ValidationPackage::new(expected_package)).into();

    // Network
    let validation_package = alice_call_data
        .network
        .get_validation_package(alice_agent_id.clone(), header_hash_priv.clone())
        .await
        .unwrap();

    assert_eq!(validation_package, expected_package);

    // Cascade
    let header_hashed = get_header(&header_hash_priv);
    let validation_package = check_cascade(&header_hashed, &alice_call_data).await;

    assert_eq!(validation_package, expected_package.0);

    // Test sub chain package

    // Commit some entries with sub chain requirements
    let header_hash = commit_some_data("create_msg", &alice_call_data, &handle).await;

    // Get the entry type
    let entry_type = alice_source_chain
        .get_element(&header_hash)
        .unwrap()
        .expect("Alice should have the entry in their authored because they just committed")
        .header()
        .entry_data()
        .unwrap()
        .1
        .clone();

    // Expecting all the elements that match this entry type from the latest to the start
    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let alice_authored = alice_source_chain.elements();
    let expected_package = alice_source_chain
        .iter_back()
        .filter_map(|shh| alice_authored.get_element(shh.header_address()))
        .filter_map(|el| {
            Ok(el.header().entry_type().cloned().and_then(|et| {
                if et == entry_type {
                    Some(el)
                } else {
                    None
                }
            }))
        })
        // Skip the actual entry
        .skip(1)
        .collect::<Vec<_>>()
        .unwrap();

    let expected_package = Some(ValidationPackage::new(expected_package)).into();

    // Network
    let validation_package = alice_call_data
        .network
        .get_validation_package(alice_agent_id.clone(), header_hash.clone())
        .await
        .unwrap();

    assert_eq!(validation_package, expected_package);

    // Cascade
    let header_hashed = get_header(&header_hash);
    let validation_package = check_cascade(&header_hashed, &alice_call_data).await;

    assert_eq!(validation_package, expected_package.0);
    conductor_test.shutdown_conductor().await;
}

#[tokio::test(threaded_scheduler)]
async fn get_agent_activity_test() {
    observability::test_run().ok();

    let zomes = vec![TestWasm::Create];
    let mut conductor_test = ConductorTestData::two_agents(zomes, false).await;
    let handle = conductor_test.handle();
    let alice_call_data = conductor_test.alice_call_data_mut();
    let alice_cell_id = &alice_call_data.cell_id;
    let alice_agent_id = alice_cell_id.agent_pubkey();
    let alice_env = alice_call_data.env.clone();

    // Helper for getting expected data
    let get_expected_full = || {
        let alice_source_chain = SourceChain::public_only(alice_env.clone().into()).unwrap();
        let valid_activity = alice_source_chain
            .iter_back()
            .collect::<Vec<_>>()
            .unwrap()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let last = valid_activity.last().cloned().unwrap();
        let status = ChainStatus::Valid(ChainHead {
            header_seq: last.header().header_seq(),
            hash: last.as_hash().clone(),
        });
        let highest_observed = Some(HighestObserved {
            header_seq: last.header().header_seq(),
            hash: vec![last.header_address().clone()],
        });

        AgentActivityResponse {
            valid_activity: ChainItems::Full(valid_activity),
            rejected_activity: ChainItems::NotRequested,
            status,
            highest_observed,
            agent: alice_agent_id.clone(),
        }
    };

    let get_expected = || {
        let mut activity = get_expected_full();
        let valid_activity = unwrap_to::unwrap_to!(activity.valid_activity => ChainItems::Full)
            .clone()
            .into_iter()
            .map(|shh| (shh.header().header_seq(), shh.header_address().clone()))
            .collect();
        activity.valid_activity = ChainItems::Hashes(valid_activity);
        activity
    };

    // Helper closure for changing to AgentActivityResponse<Element> type
    let get_expected_cascade = |activity: AgentActivityResponse| {
        let valid_activity = match activity.valid_activity {
            ChainItems::Full(headers) => ChainItems::Full(
                headers
                    .into_iter()
                    .map(|shh| Element::new(shh, None))
                    .collect(),
            ),
            ChainItems::Hashes(h) => ChainItems::Hashes(h),
            ChainItems::NotRequested => ChainItems::NotRequested,
        };
        let rejected_activity = match activity.rejected_activity {
            ChainItems::Full(headers) => ChainItems::Full(
                headers
                    .into_iter()
                    .map(|shh| Element::new(shh, None))
                    .collect(),
            ),
            ChainItems::Hashes(h) => ChainItems::Hashes(h),
            ChainItems::NotRequested => ChainItems::NotRequested,
        };
        let activity: AgentActivityResponse<Element> = AgentActivityResponse {
            agent: activity.agent,
            valid_activity,
            rejected_activity,
            status: activity.status,
            highest_observed: activity.highest_observed,
        };
        activity
    };

    commit_some_data("create_entry", &alice_call_data, &handle).await;

    // 3 ops per commit, 5 commits plus 7 for genesis + 2 for init + 2 for cap
    let mut expected_count = NUM_COMMITS * 3 + 9 + 2;

    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
    )
    .await;

    let agent_activity = alice_call_data
        .network
        .get_agent_activity(
            alice_agent_id.clone(),
            ChainQueryFilter::new(),
            GetActivityOptions {
                include_full_headers: true,
                include_valid_activity: true,
                timeout_ms: Some(GET_AGENT_ACTIVITY_TIMEOUT_MS),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(agent_activity.len(), 1);

    let mut agent_activity = alice_call_data
        .network
        .get_agent_activity(
            alice_agent_id.clone(),
            ChainQueryFilter::new(),
            GetActivityOptions {
                timeout_ms: Some(GET_AGENT_ACTIVITY_TIMEOUT_MS),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let agent_activity = agent_activity
        .pop()
        .expect("Failed to get any activity from alice");

    // Expecting every header from the latest to the beginning
    let expected_activity = get_expected();
    assert_eq!(agent_activity, expected_activity);

    let mut element_cache = ElementBuf::cache(alice_call_data.env.clone().into()).unwrap();
    let mut meta_cache = MetadataBuf::cache(alice_call_data.env.clone().into()).unwrap();
    let cache_data = DbPairMut::new(&mut element_cache, &mut meta_cache);
    let mut cascade = Cascade::empty()
        .with_cache(cache_data)
        .with_network(alice_call_data.network.clone());

    // Call the cascade without entries and check we get the headers
    let agent_activity = cascade
        .get_agent_activity(
            alice_agent_id.clone(),
            ChainQueryFilter::new(),
            GetActivityOptions {
                include_full_headers: true,
                retry_gets: 5,
                timeout_ms: Some(GET_AGENT_ACTIVITY_TIMEOUT_MS),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to get any activity from alice");

    let expected_activity = get_expected_cascade(get_expected_full());

    assert_eq!(agent_activity, expected_activity);

    let r = cascade
        .get_agent_activity(
            alice_agent_id.clone(),
            ChainQueryFilter::new().include_entries(true),
            GetActivityOptions {
                include_full_headers: true,
                retry_gets: 5,
                timeout_ms: Some(GET_AGENT_ACTIVITY_TIMEOUT_MS),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to get any activity from alice");
    let agent_activity = unwrap_to::unwrap_to!(r.valid_activity => ChainItems::Full).clone();

    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let expected_activity: Vec<_> =
        unwrap_to::unwrap_to!(get_expected_full().valid_activity => ChainItems::Full)
            .into_iter()
            .cloned()
            // We are expecting the full elements with entries
            .filter_map(|a| alice_source_chain.get_element(a.header_address()).unwrap())
            .collect();

    assert_eq!(agent_activity, expected_activity);

    // Commit private messages
    commit_some_data("create_priv_msg", &alice_call_data, &handle).await;

    alice_call_data.triggers.produce_dht_ops.trigger();

    expected_count += NUM_COMMITS * 2;
    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
    )
    .await;

    let mut agent_activity = alice_call_data
        .network
        .get_agent_activity(
            alice_agent_id.clone(),
            ChainQueryFilter::new(),
            GetActivityOptions {
                timeout_ms: Some(GET_AGENT_ACTIVITY_TIMEOUT_MS),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let agent_activity = agent_activity
        .pop()
        .expect("Failed to get any activity from alice");

    // Expecting every header from the latest to the beginning
    let expected_activity = get_expected();
    assert_eq!(agent_activity, expected_activity);

    // Commit messages
    let header_hash = commit_some_data("create_msg", &alice_call_data, &handle).await;

    // Get the entry type
    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let entry_type = alice_source_chain
        .get_element(&header_hash)
        .unwrap()
        .expect("Alice should have the entry in their authored because they just committed")
        .header()
        .entry_data()
        .unwrap()
        .1
        .clone();

    // Wait for alice to integrate the chain as an authority
    alice_call_data.triggers.produce_dht_ops.trigger();
    expected_count += NUM_COMMITS * 3;
    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
    )
    .await;

    // Call alice and get the activity
    let agent_activity = cascade
        .get_agent_activity(
            alice_agent_id.clone(),
            ChainQueryFilter::new().entry_type(entry_type.clone()),
            GetActivityOptions {
                include_full_headers: true,
                retry_gets: 5,
                timeout_ms: Some(GET_AGENT_ACTIVITY_TIMEOUT_MS),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to get any activity from alice");

    // This time we expect only activity that matches the entry type
    let mut expected_activity = get_expected_cascade(get_expected_full());
    let activity: Vec<_> =
        unwrap_to::unwrap_to!(expected_activity.valid_activity => ChainItems::Full)
            .into_iter()
            .filter(|a| {
                a.header()
                    .entry_type()
                    .map(|et| *et == entry_type)
                    .unwrap_or(false)
            })
            .cloned()
            // We are expecting the full elements with entries
            .collect();
    expected_activity.valid_activity = ChainItems::Full(activity);

    assert_eq!(agent_activity, expected_activity);

    conductor_test.shutdown_conductor().await;
}

#[tokio::test(threaded_scheduler)]
async fn get_custom_package_test() {
    observability::test_run().ok();

    let zomes = vec![TestWasm::ValidationPackageSuccess];
    let mut conductor_test = ConductorTestData::two_agents(zomes, true).await;
    let handle = conductor_test.handle();
    let alice_call_data = conductor_test.alice_call_data();
    let bob_call_data = conductor_test.bob_call_data().unwrap();
    let alice_cell_id = &alice_call_data.cell_id;

    let invocation = new_zome_call(
        &alice_cell_id,
        "commit_artist",
        (),
        TestWasm::ValidationPackageSuccess,
    )
    .unwrap();
    let result = handle.call_zome(invocation).await;

    assert_matches!(result, Err(_));

    let invocation = new_zome_call(
        &alice_cell_id,
        "commit_songs",
        (),
        TestWasm::ValidationPackageSuccess,
    )
    .unwrap();
    let result = handle.call_zome(invocation).await.unwrap().unwrap();

    assert_matches!(result, ZomeCallResponse::Ok(_));

    let invocation = new_zome_call(
        &alice_cell_id,
        "commit_artist",
        (),
        TestWasm::ValidationPackageSuccess,
    )
    .unwrap();
    let result = handle.call_zome(invocation).await.unwrap().unwrap();

    assert_matches!(result, ZomeCallResponse::Ok(_));

    // 15 for genesis plus 1 init
    // 1 artist is 3 ops.
    // and 30 songs at 6 ops each.
    let expected_count = 16 + 30 * 3 + 3;

    // Wait for bob to integrate and then check they have the package cached
    wait_for_integration(
        &bob_call_data.env,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
    )
    .await;

    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let shh = alice_source_chain
        .iter_back()
        .find(|shh| {
            Ok(shh
                .header()
                .entry_type()
                .map(|et| {
                    if let EntryType::App(aet) = et {
                        aet.id().index() == 1
                    } else {
                        false
                    }
                })
                .unwrap_or(false))
        })
        .unwrap()
        .unwrap();

    {
        let env: DbRead = bob_call_data.env.clone().into();
        let element_integrated = ElementBuf::vault(env.clone(), false).unwrap();
        let meta_integrated = MetadataBuf::vault(env.clone()).unwrap();
        let mut element_cache = ElementBuf::cache(env.clone()).unwrap();
        let mut meta_cache = MetadataBuf::cache(env.clone()).unwrap();
        let cascade = Cascade::empty()
            .with_cache(DbPairMut::new(&mut element_cache, &mut meta_cache))
            .with_integrated(DbPair::new(&element_integrated, &meta_integrated));

        let result = cascade
            .get_validation_package_local(shh.header_address())
            .unwrap();
        assert_matches!(result, Some(_));
    }

    conductor_test.shutdown_conductor().await;
}

#[tokio::test(threaded_scheduler)]
async fn get_agent_activity_host_fn_test() {
    observability::test_run().ok();

    let zomes = vec![TestWasm::Create];
    let mut conductor_test = ConductorTestData::two_agents(zomes, false).await;
    let handle = conductor_test.handle();
    let alice_call_data = conductor_test.alice_call_data();
    let alice_cell_id = &alice_call_data.cell_id;
    let alice_agent_id = alice_cell_id.agent_pubkey();
    let alice_env = alice_call_data.env.clone();

    // Helper for getting expected data
    let get_expected = || {
        let alice_source_chain = SourceChain::public_only(alice_env.clone().into()).unwrap();
        let valid_activity = alice_source_chain
            .iter_back()
            .collect::<Vec<_>>()
            .unwrap()
            .into_iter()
            .rev()
            .map(|shh| (shh.header().header_seq(), shh.header_address().clone()))
            .collect::<Vec<_>>();
        let last = valid_activity.last().cloned().unwrap();
        let status = ChainStatus::Valid(ChainHead {
            header_seq: last.0,
            hash: last.1.clone(),
        });
        let highest_observed = Some(HighestObserved {
            header_seq: last.0,
            hash: vec![last.1.clone()],
        });

        holochain_zome_types::query::AgentActivity {
            valid_activity: valid_activity,
            rejected_activity: Vec::new(),
            status,
            highest_observed,
            warrants: Vec::new(),
        }
    };

    commit_some_data("create_entry", &alice_call_data, &handle).await;

    // 3 ops per commit, 5 commits plus 7 for genesis + 2 for init
    let expected_count = NUM_COMMITS * 3 + 9;

    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
    )
    .await;

    let agent_activity = alice_call_data
        .get_api(TestWasm::Create)
        .get_agent_activity(
            alice_agent_id,
            &ChainQueryFilter::new(),
            ActivityRequest::Full,
        )
        .await;
    let expected_activity = get_expected();
    assert_eq!(agent_activity, expected_activity);

    let search = AgentActivitySearch {
        agent: alice_agent_id.clone(),
        query: ChainQueryFilter::new(),
        request: ActivityRequest::Full,
    };
    let invocation = new_zome_call(
        &alice_call_data.cell_id,
        "get_activity",
        search,
        TestWasm::Create,
    )
    .unwrap();
    let result = handle.call_zome(invocation).await.unwrap().unwrap();
    let agent_activity: holochain_zome_types::query::AgentActivity =
        unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .decode()
            .unwrap();
    assert_eq!(agent_activity, expected_activity);
    conductor_test.shutdown_conductor().await;
}

async fn commit_some_data(
    call: &str,
    alice_call_data: &CellHostFnCaller,
    handle: &ConductorHandle,
) -> HeaderHash {
    let mut header_hash = None;
    // Commit 5 entries
    for _ in 0..NUM_COMMITS {
        let invocation =
            new_zome_call(&alice_call_data.cell_id, call, (), TestWasm::Create).unwrap();
        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        let result: HeaderHash = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .decode()
            .unwrap();
        header_hash = Some(result);
    }
    header_hash.unwrap()
}

// Cascade helper function for easily getting the validation package
async fn check_cascade(
    header_hashed: &HeaderHashed,
    call_data: &CellHostFnCaller,
) -> Option<ValidationPackage> {
    let mut element_cache = ElementBuf::cache(call_data.env.clone().into()).unwrap();
    let mut meta_cache = MetadataBuf::cache(call_data.env.clone().into()).unwrap();
    let cache_data = DbPairMut::new(&mut element_cache, &mut meta_cache);
    let mut cascade = Cascade::empty()
        .with_cache(cache_data)
        .with_network(call_data.network.clone());

    // Cascade
    let validation_package = cascade
        .get_validation_package(call_data.cell_id.agent_pubkey().clone(), header_hashed)
        .await
        .unwrap();
    validation_package
}

#[tokio::test(threaded_scheduler)]
#[ignore = "Only shows a potential problem, doesn't prove something is correct"]
/// This test shows a potential slow read issue.
/// The exact same code running here in this test is 10x
/// faster then when it is run by the cell
///
/// This may not turn out to be a real issue, but this illustrates a way to reproduce this behavior,
/// and may be something we want to investigate more in the future.
async fn slow_lmdb_reads_test() {
    let num_commits = std::env::var_os("SLOW_LMDB_COMMITS")
        .and_then(|s| s.into_string().ok()?.parse::<usize>().ok())
        .unwrap_or(10);
    observability::test_run().ok();
    let zomes = vec![TestWasm::Create];
    let mut conductor_test = ConductorTestData::two_agents(zomes, false).await;
    let handle = conductor_test.handle();
    let alice_call_data = conductor_test.alice_call_data_mut();
    let alice_env = alice_call_data.env.clone();

    // Commit some data to put some load on the network
    let mut invocations = vec![];
    invocations.push(
        new_zome_call(
            &alice_call_data.cell_id,
            "create_entry",
            (),
            TestWasm::Create,
        )
        .unwrap(),
    );
    invocations
        .push(new_zome_call(&alice_call_data.cell_id, "create_msg", (), TestWasm::Create).unwrap());
    invocations.push(
        new_zome_call(
            &alice_call_data.cell_id,
            "create_priv_msg",
            (),
            TestWasm::Create,
        )
        .unwrap(),
    );
    for i in 0..num_commits {
        for invocation in &invocations {
            let r = handle.call_zome(invocation.clone()).await.unwrap().unwrap();
            assert_matches!(r, ZomeCallResponse::Ok(_));
        }
        let invocation = new_zome_call(
            &alice_call_data.cell_id,
            "create_post",
            Post(format!("{}", i)),
            TestWasm::Create,
        )
        .unwrap();
        let r = handle.call_zome(invocation.clone()).await.unwrap().unwrap();
        assert_matches!(r, ZomeCallResponse::Ok(_));
    }

    let expected_count = 9 + 3 * 3 * num_commits + 2 * num_commits;
    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
    )
    .await;
    // Get the source chain hashes
    let alice_source_chain = SourceChain::new(alice_env.clone().into()).unwrap();
    let hashes: Vec<_> = alice_source_chain
        .iter_back()
        .map(|shh| Ok(shh.header_address().clone()))
        .collect()
        .unwrap();
    let num_headers = hashes.len();

    // Time how long it takes to get the headers
    let expected_count = 9 + 3 * 2 * num_commits + 2 * num_commits;
    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
    )
    .await;

    alice_call_data
        .network
        .get_agent_activity(
            alice_call_data.cell_id.agent_pubkey().clone(),
            ChainQueryFilter::new(),
            GetActivityOptions {
                timeout_ms: Some(GET_AGENT_ACTIVITY_TIMEOUT_MS),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let mut low = u128::MAX;
    let mut high = 0;
    let mut average = 0;
    let runs = 1000;
    for _ in 0..runs {
        let element_integrated = ElementBuf::vault(alice_env.clone().into(), false).unwrap();
        let meta_integrated = MetadataBuf::vault(alice_env.clone().into()).unwrap();
        fresh_reader_test!(alice_env, |mut r| {
            let now = std::time::Instant::now();
            let hashes = meta_integrated
                .get_activity_sequence(
                    &mut r,
                    ChainItemKey::AgentStatus(
                        alice_call_data.cell_id.agent_pubkey().clone(),
                        ValidationStatus::Valid,
                    ),
                )
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            for hash in &hashes {
                element_integrated.get_header(&hash.1).unwrap();
            }
            let elapsed = now.elapsed().as_micros();
            if elapsed < low {
                low = elapsed;
            }
            if elapsed > high {
                high = elapsed;
            }
            average += elapsed;
        });
    }
    average = average / runs;
    println!("Average time to get {} headers {}us", num_headers, average);
    println!("{} us per header", average / num_headers as u128);
    println!("low {}", low / num_headers as u128);
    println!("high {}", high / num_headers as u128);
    println!("num commits {}", num_commits);

    conductor_test.shutdown_conductor().await;
}
