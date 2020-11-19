use std::{convert::TryFrom, sync::Arc};

use hdk3::prelude::{CellId, WasmError};
use holo_hash::AgentPubKey;
use holochain_keystore::AgentPubKeyExt;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::env::EnvironmentWrite;
use holochain_types::{
    app::InstalledCell,
    dna::{DnaDef, DnaFile},
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::ZomeCallResponse;
use kitsune_p2p::KitsuneP2pConfig;
use matches::assert_matches;
use tempdir::TempDir;

use crate::{
    conductor::p2p_store::all_agent_infos,
    conductor::p2p_store::inject_agent_infos,
    conductor::ConductorHandle,
    core::ribosome::error::RibosomeError,
    core::ribosome::error::RibosomeResult,
    test_utils::{install_app, new_invocation, setup_app_with_network},
};
use shrinkwraprs::Shrinkwrap;
use test_case::test_case;

const TIMEOUT_ERROR: &'static str = "inner function \'call_create_entry_remotely_hdk_extern\' failed: ZomeCallNetworkError(\"Other: timeout\")";

#[test_case(2)]
#[test_case(5)]
// #[test_case(10)] 10 works but might be too slow for our regular test run
fn conductors_call_remote(num_conductors: usize) {
    let f = async move {
        observability::test_run().ok();

        let zomes = vec![TestWasm::Create];
        let mut network = KitsuneP2pConfig::default();
        network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
            bind_to: None,
            override_host: None,
            override_port: None,
        }];
        let handles = setup(zomes, Some(network), num_conductors).await;

        init_all(&handles[..]).await;

        // 50 ms should be enough time to hit another conductor locally
        let results = call_each_other(&handles[..], 50).await;
        for (_, _, result) in results {
            match result {
                Some(r) => {
                    assert_matches!(r, Err(RibosomeError::WasmError(WasmError::Zome(e))) if e == TIMEOUT_ERROR);
                }
                // None also means a timeout which is what we want before the
                // agent info is shared
                None => (),
            }
        }

        let mut envs = Vec::with_capacity(handles.len());
        for h in &handles {
            envs.push(h.get_p2p_env().await);
        }

        exchange_peer_info(envs);

        // Give a little longer timeout here because they must find each other to pass the test
        let results = call_each_other(&handles[..], 100).await;
        for (_, _, result) in results {
            assert_matches!(result, Some(Ok(ZomeCallResponse::Ok(_))));
        }
        shutdown(handles).await;
    };
    crate::conductor::tokio_runtime().block_on(f);
}

async fn init_all(handles: &[TestHandle]) {
    let mut futures = Vec::with_capacity(handles.len());
    for h in handles.iter().cloned() {
        let f = async move {
            let invocation =
                new_invocation(&h.cell_id, "create_entry", (), TestWasm::Create).unwrap();
            h.call_zome(invocation).await.unwrap().unwrap();
        };
        let f = tokio::task::spawn(f);
        futures.push(f);
    }
    for f in futures {
        f.await.unwrap();
    }
}

async fn call_remote(a: TestHandle, b: TestHandle) -> RibosomeResult<ZomeCallResponse> {
    let invocation = new_invocation(
        &a.cell_id,
        "call_create_entry_remotely",
        b.cell_id.agent_pubkey().clone(),
        TestWasm::Create,
    )
    .unwrap();
    a.call_zome(invocation).await.unwrap()
}

async fn call_each_other(
    handles: &[TestHandle],
    timeout: u64,
) -> Vec<(usize, usize, Option<RibosomeResult<ZomeCallResponse>>)> {
    let mut results = Vec::with_capacity(handles.len() * 2);
    for (i, a) in handles.iter().cloned().enumerate() {
        let mut futures = Vec::with_capacity(handles.len());
        for (j, b) in handles.iter().cloned().enumerate() {
            // Don't call self
            if i == j {
                continue;
            }
            let f = {
                let a = a.clone();
                async move {
                    let f = call_remote(a, b);
                    // We don't want to wait the maximum network timeout
                    // in this test as it's a controlled local network
                    match tokio::time::timeout(std::time::Duration::from_millis(timeout), f).await {
                        Ok(r) => (i, j, Some(r)),
                        Err(_) => (i, j, None),
                    }
                }
            };
            // Run a set of call remotes in parallel.
            // Can't run everything in parallel or we get chain moved.
            futures.push(tokio::task::spawn(f));
        }
        for f in futures {
            results.push(f.await.unwrap());
        }
    }
    results
}

fn exchange_peer_info(envs: Vec<EnvironmentWrite>) {
    for (i, a) in envs.iter().enumerate() {
        for (j, b) in envs.iter().enumerate() {
            if i == j {
                continue;
            }
            inject_agent_infos(a.clone(), all_agent_infos(b.clone().into()).unwrap()).unwrap();
            inject_agent_infos(b.clone(), all_agent_infos(a.clone().into()).unwrap()).unwrap();
        }
    }
}

#[derive(Shrinkwrap, Clone)]
struct TestHandle {
    #[shrinkwrap(main_field)]
    handle: ConductorHandle,
    cell_id: CellId,
    __tmpdir: Arc<TempDir>,
}

impl TestHandle {
    async fn shutdown(self) {
        let shutdown = self.handle.take_shutdown_handle().await.unwrap();
        self.handle.shutdown().await;
        shutdown.await.unwrap();
    }
}

async fn shutdown(handles: Vec<TestHandle>) {
    for h in handles {
        h.shutdown().await;
    }
}

async fn setup(
    zomes: Vec<TestWasm>,
    network: Option<KitsuneP2pConfig>,
    num_conductors: usize,
) -> Vec<TestHandle> {
    let dna_file = DnaFile::new(
        DnaDef {
            name: "conductor_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: zomes.clone().into_iter().map(Into::into).collect(),
        },
        zomes.into_iter().map(Into::into),
    )
    .await
    .unwrap();

    let mut handles = Vec::with_capacity(num_conductors);
    for _ in 0..num_conductors {
        let dnas = vec![dna_file.clone()];
        let (__tmpdir, _, handle) =
            setup_app_with_network(vec![], vec![], network.clone().unwrap_or_default()).await;

        let agent_key = AgentPubKey::new_from_pure_entropy(handle.keystore())
            .await
            .unwrap();
        let cell_id = CellId::new(dna_file.dna_hash().to_owned(), agent_key.clone());
        let app = InstalledCell::new(cell_id.clone(), "cell_handle".into());
        install_app("test_app", vec![(app, None)], dnas.clone(), handle.clone()).await;
        handles.push(TestHandle {
            __tmpdir,
            cell_id,
            handle,
        });
    }
    handles
}
