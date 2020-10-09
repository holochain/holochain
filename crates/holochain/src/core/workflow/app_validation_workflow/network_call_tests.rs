use std::convert::TryInto;

use fallible_iterator::FallibleIterator;
use hdk3::prelude::{Element, ValidationPackage};
use holo_hash::HeaderHash;
use holochain_p2p::HolochainP2pCellT;
use holochain_types::chain::AgentActivityExt;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::query::{AgentActivity, ChainQueryFilter};

use crate::{
    core::state::cascade::Cascade,
    core::state::cascade::DbPairMut,
    core::state::element_buf::ElementBuf,
    core::state::metadata::MetadataBuf,
    test_utils::{
        conductor_setup::ConductorCallData, host_fn_api::*, new_invocation, wait_for_integration,
    },
};
use crate::{
    core::state::source_chain::SourceChain, test_utils::conductor_setup::ConductorTestData,
};

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

    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = std::time::Duration::from_millis(100);

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

    commit_some_data("create_entry", &alice_call_data).await;

    alice_call_data.triggers.produce_dht_ops.trigger();

    // 3 ops per commit, 5 commits plus 7 for genesis
    let mut expected_count = 3 * 5 + 7;

    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
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
    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let expected_activity = alice_source_chain
        .iter_back()
        .collect::<Vec<_>>()
        .unwrap()
        .into_iter()
        .rev()
        .collect();
    let expected_activity = AgentActivity::valid(expected_activity);
    assert_eq!(agent_activity, expected_activity);

    let mut element_cache = ElementBuf::cache(alice_call_data.env.clone().into()).unwrap();
    let mut meta_cache = MetadataBuf::cache(alice_call_data.env.clone().into()).unwrap();
    let cache_data = DbPairMut::new(&mut element_cache, &mut meta_cache);
    let mut cascade = Cascade::empty()
        .with_cache(cache_data)
        .with_network(alice_call_data.network.clone());

    // Cascade without entries
    let agent_activity = cascade
        .get_agent_activity(
            alice_agent_id.clone(),
            ChainQueryFilter::new(),
            Default::default(),
        )
        .await
        .expect("Failed to get any activity from alice");

    let expected_activity: Vec<_> = expected_activity
        .0
        .into_iter()
        .map(|shh| Element::new(shh, None))
        .collect();
    assert_eq!(agent_activity, expected_activity);

    // Cascade with entries
    let agent_activity = cascade
        .get_agent_activity(
            alice_agent_id.clone(),
            ChainQueryFilter::new().include_entries(true),
            Default::default(),
        )
        .await
        .expect("Failed to get any activity from alice");

    let expected_activity: Vec<_> = expected_activity
        .into_iter()
        .filter_map(|el| alice_source_chain.get_element(el.header_address()).unwrap())
        .collect();
    assert_eq!(agent_activity, expected_activity);

    // Commit private messages
    commit_some_data("create_priv_msg", &alice_call_data).await;

    alice_call_data.triggers.produce_dht_ops.trigger();

    expected_count += 2 * 5;
    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
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
    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let expected_activity = alice_source_chain
        .iter_back()
        .collect::<Vec<_>>()
        .unwrap()
        .into_iter()
        .rev()
        .collect();
    let expected_activity = AgentActivity::valid(expected_activity);
    assert_eq!(agent_activity, expected_activity);

    // Commit private messages
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

    alice_call_data.triggers.produce_dht_ops.trigger();
    expected_count += 3 * 5;
    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
    )
    .await;

    let mut agent_activity = alice_call_data
        .network
        .get_agent_activity(
            alice_agent_id.clone(),
            ChainQueryFilter::new().entry_type(entry_type.clone()),
            Default::default(),
        )
        .await
        .unwrap();
    let agent_activity = agent_activity
        .pop()
        .expect("Failed to get any activity from alice");

    let alice_source_chain = SourceChain::public_only(alice_call_data.env.clone().into()).unwrap();
    let expected_activity = alice_source_chain
        .iter_back()
        .filter_map(|shh| {
            Ok(shh.header().entry_type().cloned().and_then(|et| {
                if et == entry_type {
                    Some(shh)
                } else {
                    None
                }
            }))
        })
        .collect::<Vec<_>>()
        .unwrap()
        .into_iter()
        .rev()
        .collect();

    let expected_activity = AgentActivity::valid(expected_activity);
    assert_eq!(agent_activity, expected_activity);

    ConductorTestData::shutdown_conductor(handle).await;
}

async fn commit_some_data(call: &'static str, alice_call_data: &ConductorCallData) -> HeaderHash {
    let mut header_hash = None;
    // Commit 5 entries
    for _ in 0..5 {
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
