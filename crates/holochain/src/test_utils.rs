//! Utils for Holochain tests
use crate::conductor::api::AppInterfaceApi;
use crate::conductor::config::AdminInterfaceConfig;
use crate::conductor::config::ConductorConfig;
use crate::conductor::config::InterfaceDriver;
use crate::conductor::ConductorBuilder;
use crate::conductor::ConductorHandle;
use crate::core::ribosome::ZomeCallInvocation;
use crate::sweettest::SweetConductorConfig;
use crate::sweettest::SweetLocalRendezvous;
use ::fixt::prelude::*;
use hdk::prelude::ZomeName;
use holo_hash::*;
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_conductor_api::conductor::NetworkConfig;
use holochain_conductor_api::ZomeCallParamsSigned;
use holochain_keystore::MetaLairClient;
use holochain_nonce::fresh_nonce;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_state::prelude::test_db_dir;
use holochain_state::prelude::SourceChainResult;
use holochain_state::source_chain;
use holochain_types::prelude::*;
use holochain_types::test_utils::fake_dna_zomes;
use holochain_wasm_test_utils::TestWasm;
pub use itertools;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::error::Elapsed;

pub mod consistency;
pub mod host_fn_caller;
pub mod inline_zomes;

mod wait_for;
pub use wait_for::*;

/// Await consistency for warrants.
pub mod conditional_consistency;

mod big_stack_test;

use crate::sweettest::{SweetCell, SweetConductor, SweetDnaFile, SweetZome};
use crate::test_utils::host_fn_caller::HostFnCaller;
use holochain_types::websocket::AllowedOrigins;

/// Produce file and line number info at compile-time
#[macro_export]
macro_rules! here {
    ($test: expr) => {
        concat!($test, " !!!_LOOK HERE:---> ", file!(), ":", line!())
    };
}

/// Try a function, with pauses between retries, until it returns `true` or the timeout duration elapses.
/// The default timeout is 5 s.
/// The default pause is 500 ms.
pub async fn retry_fn_until_timeout<F, Fut>(
    try_fn: F,
    timeout_ms: Option<u64>,
    sleep_ms: Option<u64>,
) -> Result<(), Elapsed>
where
    F: Fn() -> Fut,
    Fut: core::future::Future<Output = bool>,
{
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms.unwrap_or(5000)),
        async {
            loop {
                if try_fn().await {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(sleep_ms.unwrap_or(500))).await;
            }
        },
    )
    .await
}

/// Retry a code block with an exit condition and then pause, until a timeout has elapsed.
/// The default timeout is 5 s.
/// The default pause is 500 ms.
#[macro_export]
macro_rules! retry_until_timeout {
    ($timeout_ms:literal, $sleep_ms:literal, $code:block) => {
        tokio::time::timeout(std::time::Duration::from_millis($timeout_ms), async {
            loop {
                $code
                tokio::time::sleep(std::time::Duration::from_millis($sleep_ms)).await;
            }
        })
        .await
        .unwrap();
    };

    ($timeout_ms:literal, $code:block) => {
        retry_until_timeout!($timeout_ms, 500, $code)
    };

    ($code:block) => {
        retry_until_timeout!(5_000, $code)
    };
}

/// Do what's necessary to install an app
pub async fn install_app(
    name: &str,
    agent: AgentPubKey,
    data: &[(DnaFile, Option<MembraneProof>)],
    conductor_handle: ConductorHandle,
) {
    conductor_handle
        .clone()
        .install_app_minimal(name.to_string(), Some(agent), data, None, None)
        .await
        .unwrap();

    conductor_handle
        .clone()
        .enable_app(name.to_string())
        .await
        .unwrap();
}

/// Payload for installing cells
pub type DnasWithProofs = Vec<(DnaFile, Option<MembraneProof>)>;

/// One of various ways to setup an app, used somewhere...
pub async fn setup_app_in_new_conductor(
    installed_app_id: InstalledAppId,
    agent: Option<AgentPubKey>,
    dnas: DnasWithProofs,
) -> (Arc<TempDir>, AppInterfaceApi, ConductorHandle, AgentPubKey) {
    let db_dir = test_db_dir();
    let conductor_handle = ConductorBuilder::new()
        .with_data_root_path(db_dir.path().to_path_buf().into())
        .test()
        .await
        .unwrap();

    let agent =
        install_app_in_conductor(conductor_handle.clone(), installed_app_id, agent, &dnas).await;

    let handle = conductor_handle.clone();

    (
        Arc::new(db_dir),
        AppInterfaceApi::new(conductor_handle),
        handle,
        agent,
    )
}

/// Install an app into an existing conductor instance
pub async fn install_app_in_conductor(
    conductor_handle: ConductorHandle,
    installed_app_id: InstalledAppId,
    agent: Option<AgentPubKey>,
    dnas_with_proofs: &[(DnaFile, Option<MembraneProof>)],
) -> AgentPubKey {
    let agent = conductor_handle
        .clone()
        .install_app_minimal(
            installed_app_id.clone(),
            agent,
            dnas_with_proofs,
            None,
            None,
        )
        .await
        .unwrap();

    conductor_handle
        .clone()
        .enable_app(installed_app_id)
        .await
        .unwrap();

    agent
}

/// Setup an app for testing
/// apps_data is a vec of app nicknames with vecs of their cell data
pub async fn setup_app_with_names(
    agent: AgentPubKey,
    apps_data: Vec<(&str, DnasWithProofs)>,
) -> (TempDir, AppInterfaceApi, ConductorHandle) {
    let dir = test_db_dir();
    let (iface, handle) =
        setup_app_inner(dir.path().to_path_buf().into(), agent, apps_data, None).await;
    (dir, iface, handle)
}

/// Setup an app with a custom network config for testing
/// apps_data is a vec of app nicknames with vecs of their cell data.
pub async fn setup_app_with_network(
    agent: AgentPubKey,
    apps_data: Vec<(&str, DnasWithProofs)>,
    network: NetworkConfig,
) -> (TempDir, AppInterfaceApi, ConductorHandle) {
    let dir = test_db_dir();
    let (iface, handle) = setup_app_inner(
        dir.path().to_path_buf().into(),
        agent,
        apps_data,
        Some(network),
    )
    .await;
    (dir, iface, handle)
}

/// Setup an app with full configurability
pub async fn setup_app_inner(
    data_root_path: DataRootPath,
    agent: AgentPubKey,
    apps_data: Vec<(&str, DnasWithProofs)>,
    _network: Option<NetworkConfig>,
) -> (AppInterfaceApi, ConductorHandle) {
    let config = ConductorConfig {
        data_root_path: Some(data_root_path.clone()),
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket {
                port: 0,
                danger_bind_addr: None,
                allowed_origins: AllowedOrigins::Any,
            },
        }]),
        ..Default::default()
    };
    let conductor_handle = ConductorBuilder::new().config(config).test().await.unwrap();

    for (app_name, cell_data) in apps_data {
        install_app(
            app_name,
            agent.clone(),
            &cell_data,
            conductor_handle.clone(),
        )
        .await;
    }

    let handle = conductor_handle.clone();

    (AppInterfaceApi::new(conductor_handle), handle)
}

/// Poll the DHT store until at least `expected` ops are integrated, or
/// panic after `num_attempts`. On failure the ops still awaiting validation are
/// listed so a failing test shows what did not progress.
pub async fn wait_for_integration(
    dht_store: &holochain_state::dht_store::DhtStore,
    expected: u64,
    num_attempts: usize,
    delay: Duration,
) {
    for _ in 0..num_attempts {
        let integrated = dht_store.as_read().count_integrated_ops().await.unwrap();
        if integrated >= expected {
            return;
        }
        tokio::time::sleep(delay).await;
    }
    let integrated = dht_store.as_read().count_integrated_ops().await.unwrap();
    panic!(
        "integration not reached: expected {expected}, integrated {integrated}\n{}",
        pending_summary(dht_store).await
    );
}

/// Assert that nothing is awaiting validation in the DHT store. On failure
/// the still-pending ops are listed so the cause is visible.
pub async fn assert_limbo_empty(dht_store: &holochain_state::dht_store::DhtStore) {
    let pending_sys = dht_store
        .as_read()
        .ops_pending_sys_validation(10_000)
        .await
        .unwrap();
    let pending_app = dht_store
        .as_read()
        .ops_pending_app_validation(10_000)
        .await
        .unwrap();
    assert!(
        pending_sys.is_empty() && pending_app.is_empty(),
        "limbo not empty: {} pending sys validation, {} pending app validation\n{}{}",
        pending_sys.len(),
        pending_app.len(),
        format_pending_ops("sys", &pending_sys),
        format_pending_ops("app", &pending_app),
    );
}

/// Summarise the ops still awaiting validation in the DHT store.
async fn pending_summary(dht_store: &holochain_state::dht_store::DhtStore) -> String {
    let pending_sys = dht_store
        .as_read()
        .ops_pending_sys_validation(10_000)
        .await
        .unwrap_or_default();
    let pending_app = dht_store
        .as_read()
        .ops_pending_app_validation(10_000)
        .await
        .unwrap_or_default();
    format!(
        "{}{}",
        format_pending_ops("sys", &pending_sys),
        format_pending_ops("app", &pending_app)
    )
}

/// Render a stage's pending ops, listing each op hash, so a failing test shows
/// exactly which ops did not progress.
fn format_pending_ops(stage: &str, ops: &[holochain_types::dht_v2::DhtOpHashed]) -> String {
    if ops.is_empty() {
        return format!("  pending {stage}-validation: none\n");
    }
    let mut out = format!("  pending {stage}-validation ({}):\n", ops.len());
    for op in ops {
        out += &format!("    {}\n", op.as_hash());
    }
    out
}

/// Show an agent's authored chain for each (agent, store) pair.
///
/// Intended for debugging in tests.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(envs)))]
pub async fn show_authored(envs: &[(AgentPubKey, &holochain_state::dht_store::DhtStore)]) {
    for (i, (author, store)) in envs.iter().enumerate() {
        let actions = store
            .as_read()
            .dump_source_chain(author)
            .await
            .expect("show_authored: dump_source_chain failed");
        for rec in &actions.records {
            let action_type = rec.action.action_type().to_string();
            let seq = rec.action.action_seq();
            let entry = rec.action.entry_hash().cloned();
            tracing::debug!(chain = %i, %seq, ?action_type, ?entry);
        }
    }
}

/// Count ops that passed validation but are not yet integrated, read from the
/// DHT store.
pub async fn get_valid_and_not_integrated_count(
    dht_store: &holochain_state::dht_store::DhtStore,
) -> usize {
    dht_store
        .as_read()
        .count_valid_not_integrated_ops()
        .await
        .unwrap() as usize
}

/// Count ops that passed validation and have been integrated, read from the
/// DHT store.
pub async fn get_valid_and_integrated_count(
    dht_store: &holochain_state::dht_store::DhtStore,
) -> usize {
    dht_store
        .as_read()
        .count_valid_integrated_ops()
        .await
        .unwrap() as usize
}

/// Helper for displaying agent infos stored on a conductor
pub async fn display_agent_infos(conductor: &ConductorHandle) {
    let all_dna_hashes = conductor.spaces.get_from_spaces(|s| (*s.dna_hash).clone());

    for dna_hash in all_dna_hashes {
        let peer_store = conductor
            .holochain_p2p()
            .peer_store(dna_hash.clone())
            .await
            .unwrap();
        let all_peers = peer_store.get_all().await.unwrap();

        for peer in all_peers {
            tracing::debug!(dna_hash = %dna_hash, ?peer);
        }
    }
}

/// Helper to create a signed zome invocation for tests
pub async fn new_zome_call<P, Z: Into<ZomeName>>(
    keystore: &MetaLairClient,
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome: Z,
) -> Result<ZomeCallParamsSigned, SerializedBytesError>
where
    P: serde::Serialize + std::fmt::Debug,
{
    let zome_call_params = new_zome_call_params(cell_id, func, payload, zome)?;
    Ok(
        ZomeCallParamsSigned::try_from_params(keystore, zome_call_params)
            .await
            .unwrap(),
    )
}

/// Helper to create an unsigned zome invocation for tests
pub fn new_zome_call_params<P, Z>(
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome: Z,
) -> Result<ZomeCallParams, SerializedBytesError>
where
    P: serde::Serialize + std::fmt::Debug,
    Z: Into<ZomeName>,
{
    let (nonce, expires_at) = fresh_nonce(Timestamp::now()).unwrap();
    Ok(ZomeCallParams {
        cell_id: cell_id.clone(),
        zome_name: zome.into(),
        cap_secret: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
        fn_name: func.into(),
        payload: ExternIO::encode(payload)?,
        provenance: cell_id.agent_pubkey().clone(),
        nonce,
        expires_at,
    })
}

/// Helper to create a zome invocation for tests
pub async fn new_invocation<P, Z>(
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome: Z,
) -> Result<ZomeCallInvocation, SerializedBytesError>
where
    P: serde::Serialize + std::fmt::Debug,
    Z: Into<Zome> + Clone,
{
    let ZomeCallParams {
        cell_id,
        cap_secret,
        fn_name,
        payload,
        provenance,
        nonce,
        expires_at,
        ..
    } = new_zome_call_params(cell_id, func, payload, zome.clone().into())?;
    Ok(ZomeCallInvocation {
        cell_id,
        zome: zome.into(),
        cap_secret,
        fn_name,
        payload: Arc::new(Mutex::new(Some(payload))),
        provenance,
        nonce,
        expires_at,
    })
}

/// A fixture example dna for unit testing.
pub fn fake_valid_dna_file(network_seed: &str) -> DnaFile {
    fake_dna_zomes(
        network_seed,
        vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
    )
}

/// Run genesis on the source chain for testing.
///
/// `dna_hash` must match the DNA the caller's `dht_db` was opened for; the
/// helper reuses it for the genesis `Action::Dna` and for the new-DB
/// `DhtStore` so the legacy and mirrored writes land in the same DNA space.
pub async fn fake_genesis(dna_hash: DnaHash, keystore: MetaLairClient) -> SourceChainResult<()> {
    fake_genesis_for_agent(dna_hash, fake_agent_pubkey_1(), keystore).await
}

/// Run genesis on the source chain for a specific agent for testing.
///
/// `dna_hash` must match the DNA the caller's `dht_db` was opened for; the
/// helper reuses it for the genesis `Action::Dna` and for the new-DB
/// `DhtStore` so the legacy and mirrored writes land in the same DNA space.
pub async fn fake_genesis_for_agent(
    dna_hash: DnaHash,
    agent: AgentPubKey,
    keystore: MetaLairClient,
) -> SourceChainResult<()> {
    let dht_store = holochain_state::test_utils::test_dht_store(dna_hash.clone()).await;

    source_chain::genesis(dht_store, keystore, dna_hash, agent, None).await
}

/// Run genesis using a caller-supplied `DhtStore`.
///
/// Use this when you need the same store for both genesis and a workspace so
/// that the workspace can read the chain head written during genesis.
pub async fn fake_genesis_with_store(
    dna_hash: DnaHash,
    keystore: MetaLairClient,
    dht_store: holochain_state::DhtStore,
) -> SourceChainResult<()> {
    fake_genesis_for_agent_with_store(dna_hash, fake_agent_pubkey_1(), keystore, dht_store).await
}

/// Run genesis for a specific agent using a caller-supplied `DhtStore`.
///
/// Use this when you need the same store for both genesis and a workspace so
/// that the workspace can read the chain head written during genesis.
pub async fn fake_genesis_for_agent_with_store(
    dna_hash: DnaHash,
    agent: AgentPubKey,
    keystore: MetaLairClient,
    dht_store: holochain_state::DhtStore,
) -> SourceChainResult<()> {
    source_chain::genesis(dht_store, keystore, dna_hash, agent, None).await
}

/// Fixture of two cells running a given TestWasm
pub struct RibosomeTestFixture {
    /// conductor running the cells
    pub conductor: SweetConductor,
    /// first cell's agent key
    pub alice_pubkey: AgentPubKey,
    /// second cell's agent key
    pub bob_pubkey: AgentPubKey,
    /// first cell's SweetZome
    pub alice: SweetZome,
    /// second cell's SweetZome
    pub bob: SweetZome,
    /// first cell's SweetCell
    pub alice_cell: SweetCell,
    /// second cell's SweetCell
    pub bob_cell: SweetCell,
    /// first cell's HostFnCaller
    pub alice_host_fn_caller: HostFnCaller,
    /// second cell's HostFnCaller
    pub bob_host_fn_caller: HostFnCaller,
}

impl RibosomeTestFixture {
    /// Create and setup the fixture with the given TestWasm
    pub async fn new(test_wasm: TestWasm) -> Self {
        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![test_wasm]).await;

        let config = SweetConductorConfig::rendezvous(true);
        let mut conductor =
            SweetConductor::from_config_rendezvous(config, SweetLocalRendezvous::new().await).await;

        let apps = conductor.setup_apps("app-", 2, [&dna_file]).await.unwrap();

        let ((alice_cell,), (bob_cell,)) = apps.into_tuples();

        let alice_host_fn_caller = HostFnCaller::create_for_zome(
            alice_cell.cell_id(),
            &conductor.raw_handle(),
            &dna_file,
            0,
        )
        .await;

        let bob_host_fn_caller = HostFnCaller::create_for_zome(
            bob_cell.cell_id(),
            &conductor.raw_handle(),
            &dna_file,
            0,
        )
        .await;

        let alice = alice_cell.zome(test_wasm);
        let bob = bob_cell.zome(test_wasm);

        let alice_pubkey = alice_cell.agent_pubkey().clone();
        let bob_pubkey = bob_cell.agent_pubkey().clone();

        retry_fn_until_timeout(
            || async { conductor.get_agent_infos(None).await.unwrap().len() == 2 },
            Some(10000),
            None,
        )
        .await
        .expect("agent infos didn't make it to the peer store");

        Self {
            conductor,
            alice_pubkey,
            bob_pubkey,
            alice,
            bob,
            alice_cell,
            bob_cell,
            alice_host_fn_caller,
            bob_host_fn_caller,
        }
    }
}
