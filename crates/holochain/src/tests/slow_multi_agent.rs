use std::convert::TryInto;

use holo_hash::HeaderHash;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{GetOutput, ZomeCallResponse};
use matches::assert_matches;
use tracing::debug;

use crate::test_utils::{
    conductor_setup::ConductorTestData, host_fn_api::Post, new_invocation, wait_for_integration,
};
use test_case::test_case;

const NUM_ATTEMPTS: usize = 100;
const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);

#[test_case(1)]
#[test_case(2)]
#[test_case(5)]
fn slow_multi_agent_get(num: usize) {
    crate::conductor::tokio_runtime().block_on(slow_multi_agent_get_inner(num));
}

async fn slow_multi_agent_get_inner(num: usize) {
    observability::test_run().ok();

    let zomes = vec![TestWasm::Create];
    let mut conductor_test = ConductorTestData::two_agents(zomes, true).await;
    let handle = conductor_test.handle();
    let alice_call_data = conductor_test.alice_call_data();
    let bob_call_data = conductor_test.bob_call_data().unwrap();

    let mut hashes_to_get: Vec<HeaderHash> = Vec::new();

    for i in 0..num {
        let post = Post(i.to_string());
        let invocation = new_invocation(
            &alice_call_data.cell_id,
            "create_post",
            post,
            TestWasm::Create,
        )
        .unwrap();
        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        let result = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .clone()
            .into_inner();
        hashes_to_get.push(result.try_into().unwrap());
    }

    // 3 ops per commit, plus 7 for genesis + 2 for init + 2 for cap
    let expected_count = num * 3 + 7 * 2 + 2 + 2;

    wait_for_integration(
        &alice_call_data.env,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
    )
    .await;

    let start = std::time::Instant::now();
    let len = hashes_to_get.len() as u64;
    for (i, hash) in hashes_to_get.into_iter().enumerate() {
        let invocation =
            new_invocation(&bob_call_data.cell_id, "get_post", hash, TestWasm::Create).unwrap();
        let this_call = std::time::Instant::now();
        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        debug!("Took {}s for call {}", this_call.elapsed().as_secs(), i);
        let result: GetOutput = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .unwrap();
        assert_matches!(result.into_inner(), Some(_));
    }
    let el = start.elapsed().as_secs();
    let average = el / len;
    debug!("Took {}s for all calls with an average of {}", el, average);
    assert_eq!(
        average, 0,
        "The average time to get an entry is greater then 1 second"
    );
    conductor_test.shutdown_conductor().await;
}
