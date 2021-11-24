//! Utils for Holochain tests

use crate::conductor::api::RealAppInterfaceApi;
use crate::conductor::api::ZomeCall;
use crate::conductor::conductor::CellStatus;
use crate::conductor::config::AdminInterfaceConfig;
use crate::conductor::config::ConductorConfig;
use crate::conductor::config::InterfaceDriver;
use crate::conductor::p2p_agent_store;
use crate::conductor::ConductorBuilder;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::ZomeCallInvocation;
use ::fixt::prelude::*;
use hdk::prelude::ZomeName;
use holo_hash::fixt::*;
use holo_hash::*;
use holochain_cascade::Cascade;
use holochain_conductor_api::IntegrationStateDump;
use holochain_conductor_api::IntegrationStateDumps;
use holochain_p2p::actor::HolochainP2pRefToCell;
use holochain_p2p::dht_arc::DhtArc;
use holochain_p2p::dht_arc::PeerViewBeta;
use holochain_p2p::event::HolochainP2pEvent;
use holochain_p2p::spawn_holochain_p2p;
use holochain_p2p::HolochainP2pCell;
use holochain_p2p::HolochainP2pRef;
use holochain_p2p::HolochainP2pSender;
use holochain_serialized_bytes::SerializedBytes;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::prelude::from_blob;
use holochain_state::prelude::test_environments;
use holochain_state::prelude::SourceChainResult;
use holochain_state::prelude::StateQueryResult;
use holochain_state::source_chain;
use holochain_state::test_utils::fresh_reader_test;
use holochain_state::test_utils::TestEnvs;
use holochain_types::prelude::*;

use holochain_p2p::DnaHashExt;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p::KitsuneP2pConfig;
use rusqlite::named_params;
use std::time::Duration;
use tokio::sync::mpsc;

pub use itertools;

use crate::sweettest::SweetCell;

pub mod conductor_setup;
pub mod consistency;
pub mod host_fn_caller;
pub mod inline_zomes;
pub mod network_simulation;

mod wait_for_any;
pub use wait_for_any::*;

/// Produce file and line number info at compile-time
#[macro_export]
macro_rules! here {
    ($test: expr) => {
        concat!($test, " !!!_LOOK HERE:---> ", file!(), ":", line!())
    };
}

/// Create metadata mocks easily by passing in
/// expected functions, return data and with_f checks
#[macro_export]
macro_rules! meta_mock {
    () => {{
        holochain_state::metadata::MockMetadataBuf::new()
    }};
    ($fun:ident) => {{
        let d: Vec<holochain_types::metadata::TimedHeaderHash> = Vec::new();
        meta_mock!($fun, d)
    }};
    ($fun:ident, $data:expr) => {{
        let mut metadata = holochain_state::metadata::MockMetadataBuf::new();
        metadata.$fun().returning({
            move |_| {
                Ok(Box::new(fallible_iterator::convert(
                    $data
                        .clone()
                        .into_iter()
                        .map(holochain_types::metadata::TimedHeaderHash::from)
                        .map(Ok),
                )))
            }
        });
        metadata
    }};
    ($fun:ident, $data:expr, $match_fn:expr) => {{
        let mut metadata = holochain_state::metadata::MockMetadataBuf::new();
        metadata.$fun().returning({
            move |a| {
                if $match_fn(a) {
                    Ok(Box::new(fallible_iterator::convert(
                        $data
                            .clone()
                            .into_iter()
                            .map(holochain_types::metadata::TimedHeaderHash::from)
                            .map(Ok),
                    )))
                } else {
                    let mut data = $data.clone();
                    data.clear();
                    Ok(Box::new(fallible_iterator::convert(
                        data.into_iter()
                            .map(holochain_types::metadata::TimedHeaderHash::from)
                            .map(Ok),
                    )))
                }
            }
        });
        metadata
    }};
}

/// A running test network with a joined cell.
/// Will shutdown on drop.
pub struct TestNetwork {
    network: Option<HolochainP2pRef>,
    respond_task: Option<tokio::task::JoinHandle<()>>,
    cell_network: HolochainP2pCell,
}

impl TestNetwork {
    /// Create a new test network
    pub fn new(
        network: HolochainP2pRef,
        respond_task: tokio::task::JoinHandle<()>,
        cell_network: HolochainP2pCell,
    ) -> Self {
        Self {
            network: Some(network),
            respond_task: Some(respond_task),
            cell_network,
        }
    }

    /// Get the holochain p2p network
    pub fn network(&self) -> HolochainP2pRef {
        self.network
            .as_ref()
            .expect("Tried to use network while it was shutting down")
            .clone()
    }

    /// Get the cell network
    pub fn cell_network(&self) -> HolochainP2pCell {
        self.cell_network.clone()
    }
}

impl Drop for TestNetwork {
    fn drop(&mut self) {
        use ghost_actor::GhostControlSender;
        let network = self.network.take().unwrap();
        let respond_task = self.respond_task.take().unwrap();
        tokio::task::spawn(async move {
            network.ghost_actor_shutdown_immediate().await.ok();
            respond_task.await.ok();
        });
    }
}

/// Convenience constructor for cell networks
pub async fn test_network(
    dna_hash: Option<DnaHash>,
    agent_key: Option<AgentPubKey>,
) -> TestNetwork {
    test_network_inner::<fn(&HolochainP2pEvent) -> bool>(dna_hash, agent_key, None).await
}

/// Convenience constructor for cell networks
/// where you need to filter some events into a channel
pub async fn test_network_with_events<F>(
    dna_hash: Option<DnaHash>,
    agent_key: Option<AgentPubKey>,
    filter: F,
    evt_send: mpsc::Sender<HolochainP2pEvent>,
) -> TestNetwork
where
    F: Fn(&HolochainP2pEvent) -> bool + Send + 'static,
{
    test_network_inner(dna_hash, agent_key, Some((filter, evt_send))).await
}

async fn test_network_inner<F>(
    dna_hash: Option<DnaHash>,
    agent_key: Option<AgentPubKey>,
    mut events: Option<(F, mpsc::Sender<HolochainP2pEvent>)>,
) -> TestNetwork
where
    F: Fn(&HolochainP2pEvent) -> bool + Send + 'static,
{
    let (network, mut recv) = spawn_holochain_p2p(
        holochain_p2p::kitsune_p2p::KitsuneP2pConfig::default(),
        holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_proxy::TlsConfig::new_ephemeral()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let respond_task = tokio::task::spawn(async move {
        use futures::future::FutureExt;
        use tokio_stream::StreamExt;
        while let Some(evt) = recv.next().await {
            if let Some((filter, tx)) = &mut events {
                if filter(&evt) {
                    tx.send(evt).await.unwrap();
                    continue;
                }
            }
            use holochain_p2p::event::HolochainP2pEvent::*;
            match evt {
                SignNetworkData { respond, .. } => {
                    respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                }
                PutAgentInfoSigned { respond, .. } => {
                    respond.r(Ok(async move { Ok(()) }.boxed().into()));
                }
                QueryAgentInfoSigned { respond, .. } => {
                    respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                }
                QueryPeerDensity { respond, .. } => {
                    respond.r(Ok(async move {
                        Ok(PeerViewBeta::new(
                            Default::default(),
                            DhtArc::full(0),
                            1.0,
                            1,
                        ))
                    }
                    .boxed()
                    .into()));
                }
                _ => {}
            }
        }
    });
    let dna = dna_hash.unwrap_or_else(|| fixt!(DnaHash));
    let mut key_fixt = AgentPubKeyFixturator::new(Predictable);
    let agent_key = agent_key.unwrap_or_else(|| key_fixt.next().unwrap());
    let cell_network = network.to_cell(dna.clone(), agent_key.clone());
    network.join(dna.clone(), agent_key).await.unwrap();
    TestNetwork::new(network, respond_task, cell_network)
}

/// Do what's necessary to install an app
pub async fn install_app(
    name: &str,
    cell_data: Vec<(InstalledCell, Option<SerializedBytes>)>,
    dnas: Vec<DnaFile>,
    conductor_handle: ConductorHandle,
) {
    for dna in dnas {
        conductor_handle.register_dna(dna).await.unwrap();
    }
    conductor_handle
        .clone()
        .install_app(name.to_string(), cell_data)
        .await
        .unwrap();

    conductor_handle
        .clone()
        .enable_app(name.to_string())
        .await
        .unwrap();

    let errors = conductor_handle
        .reconcile_cell_status_with_app_status()
        .await
        .unwrap();

    assert!(errors.is_empty(), "{:?}", errors);
}

/// Payload for installing cells
pub type InstalledCellsWithProofs = Vec<(InstalledCell, Option<SerializedBytes>)>;

/// Setup an app for testing
/// apps_data is a vec of app nicknames with vecs of their cell data
pub async fn setup_app(
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dnas: Vec<DnaFile>,
) -> (TestEnvs, RealAppInterfaceApi, ConductorHandle) {
    setup_app_inner(test_environments(), apps_data, dnas, None).await
}

/// Setup an app with a custom network config for testing
/// apps_data is a vec of app nicknames with vecs of their cell data.
pub async fn setup_app_with_network(
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dnas: Vec<DnaFile>,
    network: KitsuneP2pConfig,
) -> (TestEnvs, RealAppInterfaceApi, ConductorHandle) {
    setup_app_inner(test_environments(), apps_data, dnas, Some(network)).await
}

/// Setup an app with full configurability
pub async fn setup_app_inner(
    envs: TestEnvs,
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dnas: Vec<DnaFile>,
    network: Option<KitsuneP2pConfig>,
) -> (TestEnvs, RealAppInterfaceApi, ConductorHandle) {
    let conductor_handle = ConductorBuilder::new()
        .config(ConductorConfig {
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            network,
            ..Default::default()
        })
        .test(&envs, &[])
        .await
        .unwrap();

    for (app_name, cell_data) in apps_data {
        install_app(app_name, cell_data, dnas.clone(), conductor_handle.clone()).await;
    }

    let handle = conductor_handle.clone();

    (
        envs,
        RealAppInterfaceApi::new(conductor_handle, Default::default()),
        handle,
    )
}

/// If HC_WASM_CACHE_PATH is set warm the cache
pub fn warm_wasm_tests() {
    if let Some(_path) = std::env::var_os("HC_WASM_CACHE_PATH") {
        let wasms: Vec<_> = TestWasm::iter().collect();
        crate::fixt::RealRibosomeFixturator::new(crate::fixt::curve::Zomes(wasms))
            .next()
            .unwrap();
    }
}

/// Number of ops per sourechain change
pub struct WaitOps;

#[allow(missing_docs)]
impl WaitOps {
    pub const GENESIS: usize = 7;
    pub const INIT: usize = 2;
    pub const CAP_TOKEN: usize = 2;
    pub const ENTRY: usize = 3;
    pub const LINK: usize = 3;
    pub const DELETE_LINK: usize = 2;
    pub const UPDATE: usize = 5;
    pub const DELETE: usize = 4;

    /// Added the app but haven't made any zome calls
    /// so init hasn't happened.
    pub const fn cold_start() -> usize {
        Self::GENESIS
    }

    /// Genesis and init.
    pub const fn start() -> usize {
        Self::GENESIS + Self::INIT
    }

    /// Start but there's a cap grant in init.
    pub const fn start_with_cap() -> usize {
        Self::GENESIS + Self::INIT + Self::CAP_TOKEN
    }

    /// Path to a set depth.
    /// This doesn't take into account paths
    /// with sharding strategy.
    pub const fn path(depth: usize) -> usize {
        Self::ENTRY + (Self::LINK + Self::ENTRY) * depth
    }
}

/// Wait for all cells to reach consistency for 10 seconds
pub async fn consistency_10s(all_cells: &[&SweetCell]) {
    const NUM_ATTEMPTS: usize = 100;
    const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);
    consistency(all_cells, NUM_ATTEMPTS, DELAY_PER_ATTEMPT).await
}

/// Wait for all cells to reach consistency
#[tracing::instrument(skip(all_cells))]
pub async fn consistency(all_cells: &[&SweetCell], num_attempts: usize, delay: Duration) {
    let all_cell_envs: Vec<_> = all_cells.iter().map(|c| c.env()).collect();
    consistency_envs(&all_cell_envs[..], num_attempts, delay).await
}

/// Wait for all cell envs to reach consistency
pub async fn consistency_envs(all_cell_envs: &[&EnvWrite], num_attempts: usize, delay: Duration) {
    let mut expected_count = 0;
    for &env in all_cell_envs.iter() {
        let count = get_published_ops(env).len();
        expected_count += count;
    }
    for &env in all_cell_envs.iter() {
        wait_for_integration(env, expected_count, num_attempts, delay).await
    }
}

/// Same as wait_for_integration but with a default wait time of 60 seconds
/// Wait for all cells to reach consistency for 10 seconds
pub async fn consistency_10s_others(all_cells: &[&SweetCell]) {
    const NUM_ATTEMPTS: usize = 100;
    const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);
    consistency_others(all_cells, NUM_ATTEMPTS, DELAY_PER_ATTEMPT).await
}

/// Wait for all cells to reach consistency
#[tracing::instrument(skip(all_cells))]
pub async fn consistency_others(all_cells: &[&SweetCell], num_attempts: usize, delay: Duration) {
    let all_cell_envs: Vec<_> = all_cells.iter().map(|c| c.env()).collect();
    consistency_envs_others(&all_cell_envs[..], num_attempts, delay).await
}

async fn consistency_envs_others(
    all_cell_envs: &[&EnvWrite],
    num_attempts: usize,
    delay: Duration,
) {
    let mut expected_count = 0;
    for &env in all_cell_envs.iter() {
        let count = get_published_ops(env).len();
        expected_count += count;
    }
    let start = Some(std::time::Instant::now());
    for (i, &env) in all_cell_envs.iter().enumerate() {
        let mut others = all_cell_envs.to_vec();
        others.remove(i);
        wait_for_integration_with_others(env, &others, expected_count, num_attempts, delay, start)
            .await
    }
}

fn get_authored_ops(env: &EnvWrite) -> Vec<DhtOpLight> {
    fresh_reader_test(env.clone(), |txn| {
        txn.prepare("SELECT blob FROM DhtOp WHERE is_authored = 1")
            .unwrap()
            .query_and_then([], |row| from_blob(row.get("blob")?))
            .unwrap()
            .collect::<StateQueryResult<_>>()
            .unwrap()
    })
}

fn get_published_ops(env: &EnvWrite) -> Vec<DhtOpLight> {
    fresh_reader_test(env.clone(), |txn| {
        txn.prepare(
            "
            SELECT
            DhtOp.blob
            FROM DhtOp
            JOIN
            Header ON DhtOp.header_hash = Header.hash
            WHERE
            DhtOp.is_authored = 1
            AND
            (DhtOp.type != :store_entry OR Header.private_entry = 0)
        ",
        )
        .unwrap()
        .query_and_then(
            named_params! {
                ":store_entry": DhtOpType::StoreEntry,
            },
            |row| from_blob(row.get("blob")?),
        )
        .unwrap()
        .collect::<StateQueryResult<_>>()
        .unwrap()
    })
}

/// Same as wait_for_integration but with a default wait time of 10 seconds
#[tracing::instrument(skip(env))]
pub async fn wait_for_integration_1m(env: &EnvWrite, expected_count: usize) {
    const NUM_ATTEMPTS: usize = 120;
    const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(500);
    wait_for_integration(env, expected_count, NUM_ATTEMPTS, DELAY_PER_ATTEMPT).await
}

/// Exit early if the expected number of ops
/// have been integrated or wait for num_attempts * delay
#[tracing::instrument(skip(env))]
pub async fn wait_for_integration(
    env: &EnvWrite,
    expected_count: usize,
    num_attempts: usize,
    delay: Duration,
) {
    for i in 0..num_attempts {
        let count = display_integration(env).await;
        if count == expected_count {
            return;
        } else {
            let total_time_waited = delay * i as u32;
            tracing::debug!(?count, ?total_time_waited, counts = ?query_integration(env).await.integrated);
        }
        tokio::time::sleep(delay).await;
    }
}

fn int_ops(env: &EnvWrite) -> Vec<DhtOpLight> {
    fresh_reader_test(env.clone(), |txn| {
        txn.prepare("SELECT blob FROM DhtOp WHERE when_integrated IS NOT NULL")
            .unwrap()
            .query_and_then([], |row| from_blob(row.get("blob")?))
            .unwrap()
            .collect::<StateQueryResult<_>>()
            .unwrap()
    })
}

/// Same as wait for integration but can print other states at the same time
pub async fn wait_for_integration_with_others_10s(
    env: &EnvWrite,
    others: &[&EnvWrite],
    expected_count: usize,
    start: Option<std::time::Instant>,
) {
    const NUM_ATTEMPTS: usize = 100;
    const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);
    wait_for_integration_with_others(
        env,
        others,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT,
        start,
    )
    .await
}

#[tracing::instrument(skip(env, others, start))]
/// Same as wait for integration but can print other states at the same time
pub async fn wait_for_integration_with_others(
    env: &EnvWrite,
    others: &[&EnvWrite],
    expected_count: usize,
    num_attempts: usize,
    delay: Duration,
    start: Option<std::time::Instant>,
) {
    let mut last_total = 0;
    let this_start = std::time::Instant::now();
    for _ in 0..num_attempts {
        let count = query_integration(env).await;
        let counts = get_integration_dumps(others).await;
        let total: usize = counts.0.clone().into_iter().map(|i| i.integrated).sum();
        let num_conductors = counts.0.len() + 1;
        let total_expected = num_conductors * expected_count;
        let progress = if total_expected == 0 {
            0.0
        } else {
            total as f64 / total_expected as f64 * 100.0
        };
        let change = total.checked_sub(last_total).expect("LOST A VALUE");
        last_total = total;
        if count.integrated == expected_count {
            return;
        } else {
            let time_waited = this_start.elapsed().as_secs();
            let total_time_waited = start.map(|s| s.elapsed().as_secs()).unwrap_or(0);
            let ops_per_s = if total_time_waited == 0 {
                0.0
            } else {
                total as f64 / total_time_waited as f64
            };
            tracing::debug!(
                "Count: {}, val: {}, int: {}\nTime waited: {}s (total {}s),\nCounts: {:?}\nTotal: {} out of {} {:.4}% change:{} {:.4}ops/s\n",
                count.integrated,
                count.validation_limbo,
                count.integration_limbo,
                time_waited,
                total_time_waited,
                counts,
                total,
                total_expected,
                progress,
                change,
                ops_per_s,
            );
        }
        tokio::time::sleep(delay).await;
    }
}

#[tracing::instrument(skip(envs))]
/// Show authored data for each cell environment
pub fn show_authored(envs: &[&EnvWrite]) {
    for (i, &env) in envs.iter().enumerate() {
        fresh_reader_test(env.clone(), |txn| {
            txn.prepare("SELECT DISTINCT Header.seq, Header.type, Header.entry_hash FROM Header JOIN DhtOp ON Header.hash = DhtOp.hash WHERE is_authored = 1")
            .unwrap()
            .query_map([], |row| {
                let header_type: String = row.get("type")?;
                let seq: u32 = row.get("seq")?;
                let entry: Option<EntryHash> = row.get("entry_hash")?;
                Ok((header_type, seq, entry))
            })
            .unwrap()
            .for_each(|r|{
                let (header_type, seq, entry) = r.unwrap();
                tracing::debug!(chain = %i, %seq, ?header_type, ?entry);
            });
        });
    }
}

#[tracing::instrument(skip(envs))]
/// Show authored op data for each cell environment
pub async fn show_authored_ops(envs: &[&EnvWrite]) {
    let mut all_auth = Vec::new();
    for (i, env) in envs.iter().enumerate() {
        let auth = get_authored_ops(env);
        all_auth.extend(auth.clone());
        for (j, op) in auth.iter().enumerate() {
            tracing::debug!(chain = %i, op_num = %j, ?op);
        }
    }
    for (i, env) in envs.iter().enumerate() {
        let int = int_ops(env);
        for op in &all_auth {
            if !int.iter().any(|a| a == op) {
                tracing::warn!(is_missing = ?op, for_env = %i);
                show_data(env, op).await;
            }
        }
    }
}

async fn show_data(env: &EnvWrite, op: &DhtOpLight) {
    let mut cascade = Cascade::empty().with_vault(env.clone().into());
    if let Some(el) = cascade
        .retrieve(op.header_hash().clone().into(), Default::default())
        .await
        .unwrap()
    {
        let header = el.header();
        let entry = format!("{:?}", el.entry());
        tracing::debug!(seq_num = %header.header_seq(), header_type = ?header.header_type(), op_type = %op.to_string(), ?entry);
    }
}

async fn get_integration_dumps(envs: &[&EnvWrite]) -> IntegrationStateDumps {
    let mut output = Vec::new();
    for env in envs {
        let env = *env;
        output.push(query_integration(env).await);
    }
    IntegrationStateDumps(output)
}

async fn query_integration(env: &EnvWrite) -> IntegrationStateDump {
    crate::conductor::integration_dump(&env.clone().into())
        .await
        .unwrap()
}

async fn display_integration(env: &EnvWrite) -> usize {
    fresh_reader_test(env.clone(), |txn| {
        txn.query_row(
            "SELECT COUNT(hash) FROM DhtOp WHERE DhtOp.when_integrated IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap()
    })
}

/// Helper for displaying agent infos stored on a conductor
pub async fn display_agent_infos(conductor: &ConductorHandle) {
    for cell_id in conductor.list_cell_ids(Some(CellStatus::Joined)) {
        let space = cell_id.dna_hash().to_kitsune();
        let env = conductor.get_p2p_env(space);
        let info = p2p_agent_store::dump_state(env.into(), Some(cell_id))
            .await
            .unwrap();
        tracing::debug!(%info);
    }
}

/// Helper to create a zome invocation for tests
pub fn new_zome_call<P, Z: Into<ZomeName>>(
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome: Z,
) -> Result<ZomeCall, SerializedBytesError>
where
    P: serde::Serialize + std::fmt::Debug,
{
    Ok(ZomeCall {
        cell_id: cell_id.clone(),
        zome_name: zome.into(),
        cap_secret: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
        fn_name: func.into(),
        payload: ExternIO::encode(payload)?,
        provenance: cell_id.agent_pubkey().clone(),
    })
}

/// Helper to create a zome invocation for tests
pub fn new_invocation<P, Z: Into<Zome>>(
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome: Z,
) -> Result<ZomeCallInvocation, SerializedBytesError>
where
    P: serde::Serialize + std::fmt::Debug,
{
    Ok(ZomeCallInvocation {
        cell_id: cell_id.clone(),
        zome: zome.into(),
        cap_secret: Some(CapSecretFixturator::new(Unpredictable).next().unwrap()),
        fn_name: func.into(),
        payload: ExternIO::encode(payload)?,
        provenance: cell_id.agent_pubkey().clone(),
    })
}

/// A fixture example dna for unit testing.
pub fn fake_valid_dna_file(uid: &str) -> DnaFile {
    fake_dna_zomes(uid, vec![(TestWasm::Foo.into(), TestWasm::Foo.into())])
}

/// Run genesis on the source chain for testing.
pub async fn fake_genesis(vault: EnvWrite) -> SourceChainResult<()> {
    fake_genesis_for_agent(vault, fake_agent_pubkey_1()).await
}

/// Run genesis on the source chain for a specific agent for testing.
pub async fn fake_genesis_for_agent(vault: EnvWrite, agent: AgentPubKey) -> SourceChainResult<()> {
    let dna = fake_dna_file("cool dna");
    let dna_hash = dna.dna_hash().clone();

    source_chain::genesis(vault, dna_hash, agent, None).await
}

/// Force all dht ops without enough validation receipts to be published.
pub async fn force_publish_dht_ops(
    vault: &EnvWrite,
    publish_trigger: &mut TriggerSender,
) -> DatabaseResult<()> {
    vault
        .async_commit(|txn| {
            DatabaseResult::Ok(txn.execute(
                "UPDATE DhtOp SET last_publish_time = NULL WHERE receipts_complete IS NULL",
                [],
            )?)
        })
        .await?;
    publish_trigger.trigger();
    Ok(())
}
