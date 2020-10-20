use std::convert::TryInto;

use fallible_iterator::FallibleIterator;
use hdk3::prelude::{Element, ValidationPackage};
use holo_hash::HeaderHash;
use holochain_p2p::{actor::GetActivityOptions, HolochainP2pCellT};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::query::{
    Activity, AgentActivity, ChainHead, ChainQueryFilter, ChainStatus, HighestObserved,
};

use crate::{
    core::state::cascade::Cascade,
    core::state::cascade::DbPairMut,
    core::state::element_buf::ElementBuf,
    core::state::metadata::MetadataBuf,
    core::state::metadata::MetadataBufT,
    test_utils::{
        conductor_setup::ConductorCallData, host_fn_api::*, new_invocation, wait_for_integration,
    },
};
use crate::{
    core::state::source_chain::SourceChain, test_utils::conductor_setup::ConductorTestData,
};

const NUM_COMMITS: usize = 1;
// Check if the correct number of ops are integrated
// every 100 ms for a maximum of 10 seconds but early exit
// if they are there.
const NUM_ATTEMPTS: usize = 100;
const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);

#[tokio::test(threaded_scheduler)]
async fn get_validation_package_test() {
    observability::test_run().ok();

    let zomes = vec![TestWasm::Create];
    let conductor_test = ConductorTestData::new(zomes, false).await;
    let ConductorTestData {
        __tmpdir,
        handle,
        mut alice_call_data,
        ..
    } = conductor_test;
    let alice_cell_id = &alice_call_data.cell_id;
    let alice_agent_id = alice_cell_id.agent_pubkey();

    let header_hash = commit_some_data("create_entry", &alice_call_data).await;

    let validation_package = alice_call_data
        .network
        .get_validation_package(alice_agent_id.clone(), header_hash.clone())
        .await
        .unwrap();

    // Expecting every header from the latest to the beginning
    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let alice_authored = alice_source_chain.elements();
    let expected_package = alice_source_chain
        .iter_back()
        .filter_map(|shh| alice_authored.get_element(shh.header_address()))
        .collect::<Vec<_>>()
        .unwrap();
    let expected_package = Some(ValidationPackage::new(expected_package)).into();
    assert_eq!(validation_package, expected_package);

    // What happens if we commit a private entry?
    let header_hash_priv = commit_some_data("create_priv_msg", &alice_call_data).await;

    // Check we still get the last package with new commits
    let validation_package = alice_call_data
        .network
        .get_validation_package(alice_agent_id.clone(), header_hash)
        .await
        .unwrap();
    assert_eq!(validation_package, expected_package);

    // Get the package for the private entry, this is still full chain
    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let alice_authored = alice_source_chain.elements();
    let validation_package = alice_call_data
        .network
        .get_validation_package(alice_agent_id.clone(), header_hash_priv)
        .await
        .unwrap();

    let expected_package = alice_source_chain
        .iter_back()
        .filter_map(|shh| alice_authored.get_element(shh.header_address()))
        .collect::<Vec<_>>()
        .unwrap();

    let expected_package = Some(ValidationPackage::new(expected_package)).into();
    assert_eq!(validation_package, expected_package);

    // Test sub chain package

    // Commit some entries with sub chain requirements
    let header_hash = commit_some_data("create_msg", &alice_call_data).await;

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

    let validation_package = alice_call_data
        .network
        .get_validation_package(alice_agent_id.clone(), header_hash)
        .await
        .unwrap();

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
        .collect::<Vec<_>>()
        .unwrap();

    let expected_package = Some(ValidationPackage::new(expected_package)).into();
    assert_eq!(validation_package, expected_package);

    ConductorTestData::shutdown_conductor(handle).await;
}

#[tokio::test(threaded_scheduler)]
async fn get_agent_activity_test() {
    observability::test_run().ok();

    let zomes = vec![TestWasm::Create];
    let conductor_test = ConductorTestData::new(zomes, false).await;
    let ConductorTestData {
        __tmpdir,
        handle,
        mut alice_call_data,
        ..
    } = conductor_test;
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

        AgentActivity {
            valid_activity: Activity::Full(valid_activity),
            rejected_activity: Activity::NotRequested,
            status,
            highest_observed,
            agent: alice_agent_id.clone(),
        }
    };

    let get_expected = || {
        let mut activity = get_expected_full();
        let valid_activity = unwrap_to::unwrap_to!(activity.valid_activity => Activity::Full)
            .clone()
            .into_iter()
            .map(|shh| (shh.header().header_seq(), shh.header_address().clone()))
            .collect();
        activity.valid_activity = Activity::Hashes(valid_activity);
        activity
    };

    // Helper closure for changing to AgentActivity<Element> type
    let get_expected_cascade = |activity: AgentActivity| {
        let valid_activity = match activity.valid_activity {
            Activity::Full(headers) => Activity::Full(
                headers
                    .into_iter()
                    .map(|shh| Element::new(shh, None))
                    .collect(),
            ),
            Activity::Hashes(h) => Activity::Hashes(h),
            Activity::NotRequested => Activity::NotRequested,
        };
        let rejected_activity = match activity.rejected_activity {
            Activity::Full(headers) => Activity::Full(
                headers
                    .into_iter()
                    .map(|shh| Element::new(shh, None))
                    .collect(),
            ),
            Activity::Hashes(h) => Activity::Hashes(h),
            Activity::NotRequested => Activity::NotRequested,
        };
        let activity: AgentActivity<Element> = AgentActivity {
            agent: activity.agent,
            valid_activity,
            rejected_activity,
            status: activity.status,
            highest_observed: activity.highest_observed,
        };
        activity
    };

    commit_some_data("create_entry", &alice_call_data).await;

    alice_call_data.triggers.produce_dht_ops.trigger();

    // 3 ops per commit, 5 commits plus 7 for genesis
    let mut expected_count = NUM_COMMITS * 3 + 7;

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
            Default::default(),
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
                ..Default::default()
            },
        )
        .await
        .expect("Failed to get any activity from alice");

    let expected_activity = get_expected_cascade(get_expected_full());

    assert_eq!(agent_activity, expected_activity);

    // Call the cascade and check we can get the entries as well
    // Parallel gets to the same agent can overwhelm them so we
    // need to try a few time
    let mut agent_activity = None;
    for _ in 0..100 {
        let r = cascade
            .get_agent_activity(
                alice_agent_id.clone(),
                ChainQueryFilter::new().include_entries(true),
                GetActivityOptions {
                    include_full_headers: true,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to get any activity from alice");
        match r.valid_activity {
            Activity::Full(h) if !h.is_empty() => {
                agent_activity = Some(h);
                break;
            }
            _ => (),
        }
        tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
    }
    let agent_activity = agent_activity.expect("Failed to get any activity from alice");

    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let expected_activity: Vec<_> =
        unwrap_to::unwrap_to!(get_expected_full().valid_activity => Activity::Full)
            .into_iter()
            .cloned()
            // We are expecting the full elements with entries
            .filter_map(|a| alice_source_chain.get_element(a.header_address()).unwrap())
            .collect();

    assert_eq!(agent_activity, expected_activity);

    // Commit private messages
    commit_some_data("create_priv_msg", &alice_call_data).await;

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
            Default::default(),
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
    let header_hash = commit_some_data("create_msg", &alice_call_data).await;

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
                ..Default::default()
            },
        )
        .await
        .expect("Failed to get any activity from alice");

    // This time we expect only activity that matches the entry type
    let mut expected_activity = get_expected_cascade(get_expected_full());
    let activity: Vec<_> =
        unwrap_to::unwrap_to!(expected_activity.valid_activity => Activity::Full)
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
    expected_activity.valid_activity = Activity::Full(activity);

    assert_eq!(agent_activity, expected_activity);

    ConductorTestData::shutdown_conductor(handle).await;
}

async fn commit_some_data(call: &'static str, alice_call_data: &ConductorCallData) -> HeaderHash {
    let mut header_hash = None;
    // Commit 5 entries
    for _ in 0..NUM_COMMITS {
        let invocation =
            new_invocation(&alice_call_data.cell_id, call, (), TestWasm::Create).unwrap();
        header_hash = Some(
            call_zome_direct(
                &alice_call_data.env,
                alice_call_data.call_data(TestWasm::Create),
                invocation,
            )
            .await
            .try_into()
            .unwrap(),
        );
    }
    header_hash.unwrap()
}
