//! Utils for Holochain tests
use crate::conductor::api::RealAppInterfaceApi;
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
use holochain_conductor_api::IntegrationStateDump;
use holochain_conductor_api::IntegrationStateDumps;
use holochain_conductor_api::ZomeCall;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_p2p::dht::prelude::Topology;
use holochain_p2p::dht::ArqStrat;
use holochain_p2p::dht::PeerViewQ;
use holochain_p2p::event::HolochainP2pEvent;
use holochain_p2p::spawn_holochain_p2p;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pRef;
use holochain_p2p::HolochainP2pSender;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::nonce::fresh_nonce;
use holochain_state::prelude::from_blob;
use holochain_state::prelude::test_db_dir;
use holochain_state::prelude::SourceChainResult;
use holochain_state::prelude::StateQueryResult;
use holochain_state::source_chain;
use holochain_state::test_utils::fresh_reader_test;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p::KitsuneP2pConfig;
use kitsune_p2p_types::ok_fut;
use rusqlite::named_params;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::mpsc;

pub use itertools;

pub mod conductor_setup;
pub mod consistency;
pub mod host_fn_caller;
pub mod inline_zomes;
pub mod network_simulation;

mod wait_for;
pub use wait_for::*;

pub use crate::sweettest::sweet_consistency::*;

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
        let d: Vec<holochain_types::metadata::TimedActionHash> = Vec::new();
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
                        .map(holochain_types::metadata::TimedActionHash::from)
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
                            .map(holochain_types::metadata::TimedActionHash::from)
                            .map(Ok),
                    )))
                } else {
                    let mut data = $data.clone();
                    data.clear();
                    Ok(Box::new(fallible_iterator::convert(
                        data.into_iter()
                            .map(holochain_types::metadata::TimedActionHash::from)
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
    dna_network: HolochainP2pDna,

    /// List of arguments used for `check_op_data` calls
    #[allow(clippy::type_complexity)]
    pub check_op_data_calls: Arc<
        std::sync::Mutex<
            Vec<(
                kitsune_p2p_types::KSpace,
                Vec<kitsune_p2p_types::KOpHash>,
                Option<kitsune_p2p::dependencies::kitsune_p2p_fetch::FetchContext>,
            )>,
        >,
    >,
}

impl TestNetwork {
    /// Create a new test network
    #[allow(clippy::type_complexity)]
    fn new(
        network: HolochainP2pRef,
        respond_task: tokio::task::JoinHandle<()>,
        dna_network: HolochainP2pDna,
        check_op_data_calls: Arc<
            std::sync::Mutex<
                Vec<(
                    kitsune_p2p_types::KSpace,
                    Vec<kitsune_p2p_types::KOpHash>,
                    Option<kitsune_p2p::dependencies::kitsune_p2p_fetch::FetchContext>,
                )>,
            >,
        >,
    ) -> Self {
        Self {
            network: Some(network),
            respond_task: Some(respond_task),
            dna_network,
            check_op_data_calls,
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
    pub fn dna_network(&self) -> HolochainP2pDna {
        self.dna_network.clone()
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
    let mut config = holochain_p2p::kitsune_p2p::KitsuneP2pConfig::default();
    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.tx2_implicit_timeout_ms = 500;
    let tuning = std::sync::Arc::new(tuning);
    let cutoff = tuning.danger_gossip_recent_threshold();
    config.tuning_params = tuning;

    let check_op_data_calls = Arc::new(std::sync::Mutex::new(Vec::new()));

    let test_host = {
        let check_op_data_calls = check_op_data_calls.clone();
        kitsune_p2p::HostStub::with_check_op_data(Box::new(move |space, list, ctx| {
            let out = list.iter().map(|_| false).collect();
            check_op_data_calls.lock().unwrap().push((space, list, ctx));
            futures::FutureExt::boxed(async move { Ok(out) }).into()
        }))
    };

    let (network, mut recv) = spawn_holochain_p2p(
        config,
        holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_types::tls::TlsConfig::new_ephemeral(
        )
        .await
        .unwrap(),
        test_host,
    )
    .await
    .unwrap();
    let respond_task = tokio::task::spawn(async move {
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
                    respond.r(ok_fut(Ok([0; 64].into())));
                }
                PutAgentInfoSigned { respond, .. } => {
                    respond.r(ok_fut(Ok(())));
                }
                QueryAgentInfoSigned { respond, .. } => {
                    respond.r(ok_fut(Ok(vec![])));
                }
                QueryAgentInfoSignedNearBasis { respond, .. } => {
                    respond.r(ok_fut(Ok(vec![])));
                }
                QueryGossipAgents { respond, .. } => {
                    respond.r(ok_fut(Ok(vec![])));
                }
                QueryPeerDensity { respond, .. } => {
                    respond.r(ok_fut(Ok(PeerViewQ::new(
                        Topology::standard_epoch(cutoff),
                        ArqStrat::default(),
                        vec![],
                    )
                    .into())));
                }
                oth => tracing::warn!(?oth, "UnhandledEvent"),
            }
        }
    });
    let dna = dna_hash.unwrap_or_else(|| fixt!(DnaHash));
    let mut key_fixt = AgentPubKeyFixturator::new(Predictable);
    let agent_key = agent_key.unwrap_or_else(|| key_fixt.next().unwrap());
    let dna_network = network.to_dna(dna.clone(), None);
    network
        .join(dna.clone(), agent_key, None, None)
        .await
        .unwrap();
    TestNetwork::new(network, respond_task, dna_network, check_op_data_calls)
}

/// Do what's necessary to install an app
pub async fn install_app(
    name: &str,
    cell_data: Vec<(InstalledCell, Option<MembraneProof>)>,
    dnas: Vec<DnaFile>,
    conductor_handle: ConductorHandle,
) {
    for dna in dnas {
        conductor_handle.register_dna(dna).await.unwrap();
    }
    conductor_handle
        .clone()
        .install_app_legacy(name.to_string(), cell_data)
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
pub type InstalledCellsWithProofs = Vec<(InstalledCell, Option<MembraneProof>)>;

/// One of various ways to setup an app, used somewhere...
pub async fn setup_app_in_new_conductor(
    installed_app_id: InstalledAppId,
    dnas: Vec<DnaFile>,
    cell_data: Vec<(InstalledCell, Option<MembraneProof>)>,
) -> (Arc<TempDir>, RealAppInterfaceApi, ConductorHandle) {
    let db_dir = test_db_dir();

    let conductor_handle = ConductorBuilder::new()
        .test(db_dir.path(), &[])
        .await
        .unwrap();

    install_app_in_conductor(conductor_handle.clone(), installed_app_id, dnas, cell_data).await;

    let handle = conductor_handle.clone();

    (
        Arc::new(db_dir),
        RealAppInterfaceApi::new(conductor_handle),
        handle,
    )
}

/// Install an app into an existing conductor instance
pub async fn install_app_in_conductor(
    conductor_handle: ConductorHandle,
    installed_app_id: InstalledAppId,
    dnas: Vec<DnaFile>,
    cell_data: Vec<(InstalledCell, Option<MembraneProof>)>,
) {
    for dna in dnas {
        conductor_handle.register_dna(dna).await.unwrap();
    }

    conductor_handle
        .clone()
        .install_app_legacy(installed_app_id.clone(), cell_data)
        .await
        .unwrap();

    conductor_handle
        .clone()
        .enable_app(installed_app_id)
        .await
        .unwrap();

    let errors = conductor_handle
        .clone()
        .reconcile_cell_status_with_app_status()
        .await
        .unwrap();

    assert!(errors.is_empty());
}

/// Setup an app for testing
/// apps_data is a vec of app nicknames with vecs of their cell data
pub async fn setup_app_with_names(
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dnas: Vec<DnaFile>,
) -> (TempDir, RealAppInterfaceApi, ConductorHandle) {
    let dir = test_db_dir();
    let (iface, handle) = setup_app_inner(dir.path(), apps_data, dnas, None).await;
    (dir, iface, handle)
}

/// Setup an app with a custom network config for testing
/// apps_data is a vec of app nicknames with vecs of their cell data.
pub async fn setup_app_with_network(
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dnas: Vec<DnaFile>,
    network: KitsuneP2pConfig,
) -> (TempDir, RealAppInterfaceApi, ConductorHandle) {
    let dir = test_db_dir();
    let (iface, handle) = setup_app_inner(dir.path(), apps_data, dnas, Some(network)).await;
    (dir, iface, handle)
}

/// Setup an app with full configurability
pub async fn setup_app_inner(
    db_dir: &Path,
    apps_data: Vec<(&str, InstalledCellsWithProofs)>,
    dnas: Vec<DnaFile>,
    network: Option<KitsuneP2pConfig>,
) -> (RealAppInterfaceApi, ConductorHandle) {
    let conductor_handle = ConductorBuilder::new()
        .config(ConductorConfig {
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            network,
            ..Default::default()
        })
        .test(db_dir, &[])
        .await
        .unwrap();

    for (app_name, cell_data) in apps_data {
        install_app(app_name, cell_data, dnas.clone(), conductor_handle.clone()).await;
    }

    let handle = conductor_handle.clone();

    (RealAppInterfaceApi::new(conductor_handle), handle)
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

/// Wait for all cell envs to reach consistency
pub async fn consistency_dbs<AuthorDb, DhtDb>(
    all_cell_dbs: &[(&AgentPubKey, &AuthorDb, Option<&DhtDb>)],
    num_attempts: usize,
    delay: Duration,
) where
    AuthorDb: ReadAccess<DbKindAuthored>,
    DhtDb: ReadAccess<DbKindDht>,
{
    let mut expected_count = 0;
    for (author, db) in all_cell_dbs.iter().map(|(author, a, _)| (author, a)) {
        let count = get_published_ops(*db, author).len();
        expected_count += count;
    }
    for &db in all_cell_dbs.iter().flat_map(|(_, _, d)| d) {
        wait_for_integration(db, expected_count, num_attempts, delay).await
    }
}

/// Alternate version of consistency awaiting (TODO: what is this actually doing?)
pub(crate) async fn consistency_dbs_others<AuthorDb, DhtDb>(
    all_cell_dbs: &[(&AgentPubKey, &AuthorDb, &DhtDb)],
    num_attempts: usize,
    delay: Duration,
) where
    AuthorDb: ReadAccess<DbKindAuthored>,
    DhtDb: ReadAccess<DbKindDht>,
{
    let mut expected_count = 0;
    for (author, db) in all_cell_dbs.iter().map(|(author, a, _)| (author, a)) {
        let count = get_published_ops(*db, author).len();
        expected_count += count;
    }
    let start = Some(std::time::Instant::now());
    for (i, &db) in all_cell_dbs.iter().map(|(_, _, d)| d).enumerate() {
        let mut others: Vec<_> = all_cell_dbs.iter().map(|(_, _, d)| *d).collect();
        others.remove(i);
        wait_for_integration_with_others(db, &others, expected_count, num_attempts, delay, start)
            .await
    }
}

fn get_published_ops<Db: ReadAccess<DbKindAuthored>>(
    db: &Db,
    author: &AgentPubKey,
) -> Vec<DhtOpLight> {
    fresh_reader_test(db.clone(), |txn| {
        txn.prepare(
            "
            SELECT
            DhtOp.type, Action.hash, Action.blob
            FROM DhtOp
            JOIN
            Action ON DhtOp.action_hash = Action.hash
            WHERE
            Action.author = :author
            AND (DhtOp.type != :store_entry OR Action.private_entry = 0)
        ",
        )
        .unwrap()
        .query_and_then(
            named_params! {
                ":store_entry": DhtOpType::StoreEntry,
                ":author": author,
            },
            |row| {
                let op_type: DhtOpType = row.get("type")?;
                let hash: ActionHash = row.get("hash")?;
                let action: SignedAction = from_blob(row.get("blob")?)?;
                Ok(DhtOpLight::from_type(op_type, hash, &action.0)?)
            },
        )
        .unwrap()
        .collect::<StateQueryResult<_>>()
        .unwrap()
    })
}

/// Same as wait_for_integration but with a default wait time of 10 seconds
#[tracing::instrument(skip(db))]
pub async fn wait_for_integration_1m<Db: ReadAccess<DbKindDht>>(db: &Db, expected_count: usize) {
    const NUM_ATTEMPTS: usize = 120;
    const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(500);
    wait_for_integration(db, expected_count, NUM_ATTEMPTS, DELAY_PER_ATTEMPT).await
}

/// Exit early if the expected number of ops
/// have been integrated or wait for num_attempts * delay
#[tracing::instrument(skip(db))]
pub async fn wait_for_integration<Db: ReadAccess<DbKindDht>>(
    db: &Db,
    expected_count: usize,
    num_attempts: usize,
    delay: Duration,
) {
    for i in 0..num_attempts {
        let count = display_integration(db);
        if count >= expected_count {
            if count > expected_count {
                tracing::warn!("count > expected_count, meaning you may not be accounting for all nodes in this test.
                Consistency may not be complete.")
            }
            return;
        } else {
            let total_time_waited = delay * i as u32;
            tracing::debug!(?count, ?total_time_waited, counts = ?query_integration(db).await);
        }
        tokio::time::sleep(delay).await;
    }

    panic!("Consistency not achieved after {} attempts", num_attempts);
}

/// Same as wait for integration but can print other states at the same time
pub async fn wait_for_integration_with_others_10s<Db: ReadAccess<DbKindDht>>(
    db: &Db,
    others: &[&Db],
    expected_count: usize,
    start: Option<std::time::Instant>,
) {
    const NUM_ATTEMPTS: usize = 100;
    const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);
    wait_for_integration_with_others(
        db,
        others,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT,
        start,
    )
    .await
}

#[tracing::instrument(skip(db, others, start))]
/// Same as wait for integration but can print other states at the same time
pub async fn wait_for_integration_with_others<Db: ReadAccess<DbKindDht>>(
    db: &Db,
    others: &[&Db],
    expected_count: usize,
    num_attempts: usize,
    delay: Duration,
    start: Option<std::time::Instant>,
) {
    let mut last_total = 0;
    let this_start = std::time::Instant::now();
    for _ in 0..num_attempts {
        let count = query_integration(db).await;
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
        if count.integrated >= expected_count {
            if count.integrated > expected_count {
                tracing::warn!("count > expected_count, meaning you may not be accounting for all nodes in this test.
                Consistency may not be complete.")
            }
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

    panic!(
        "Integration with others not complete after {} attempts",
        num_attempts
    );
}

#[tracing::instrument(skip(envs))]
/// Show authored data for each cell environment
pub fn show_authored<Db: ReadAccess<DbKindAuthored>>(envs: &[&Db]) {
    for (i, &db) in envs.iter().enumerate() {
        fresh_reader_test(db.clone(), |txn| {
            txn.prepare("SELECT DISTINCT Action.seq, Action.type, Action.entry_hash FROM Action JOIN DhtOp ON Action.hash = DhtOp.hash")
            .unwrap()
            .query_map([], |row| {
                let action_type: String = row.get("type")?;
                let seq: u32 = row.get("seq")?;
                let entry: Option<EntryHash> = row.get("entry_hash")?;
                Ok((action_type, seq, entry))
            })
            .unwrap()
            .for_each(|r|{
                let (action_type, seq, entry) = r.unwrap();
                tracing::debug!(chain = %i, %seq, ?action_type, ?entry);
            });
        });
    }
}

async fn get_integration_dumps<Db: ReadAccess<DbKindDht>>(dbs: &[&Db]) -> IntegrationStateDumps {
    let mut output = Vec::new();
    for db in dbs {
        let db = *db;
        output.push(query_integration(db).await);
    }
    IntegrationStateDumps(output)
}

/// Show the current db state.
pub async fn query_integration<Db: ReadAccess<DbKindDht>>(db: &Db) -> IntegrationStateDump {
    crate::conductor::integration_dump(&db.clone().into())
        .await
        .unwrap()
}

fn display_integration<Db: ReadAccess<DbKindDht>>(db: &Db) -> usize {
    fresh_reader_test(db.clone(), |txn| {
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
    for cell_id in conductor.running_cell_ids(Some(CellStatus::Joined)) {
        let space = cell_id.dna_hash();
        let db = conductor.get_p2p_db(space);
        let info = p2p_agent_store::dump_state(db.into(), Some(cell_id))
            .await
            .unwrap();
        tracing::debug!(%info);
    }
}

/// Helper to create a signed zome invocation for tests
pub async fn new_zome_call<P, Z: Into<ZomeName>>(
    keystore: &MetaLairClient,
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome: Z,
) -> Result<ZomeCall, SerializedBytesError>
where
    P: serde::Serialize + std::fmt::Debug,
{
    let zome_call_unsigned = new_zome_call_unsigned(cell_id, func, payload, zome)?;
    Ok(
        ZomeCall::try_from_unsigned_zome_call(keystore, zome_call_unsigned)
            .await
            .unwrap(),
    )
}

/// Helper to create an unsigned zome invocation for tests
pub fn new_zome_call_unsigned<P, Z: Into<ZomeName>>(
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome: Z,
) -> Result<ZomeCallUnsigned, SerializedBytesError>
where
    P: serde::Serialize + std::fmt::Debug,
{
    let (nonce, expires_at) = fresh_nonce(Timestamp::now()).unwrap();
    Ok(ZomeCallUnsigned {
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
pub async fn new_invocation<P, Z: Into<Zome> + Clone>(
    keystore: &MetaLairClient,
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome: Z,
) -> Result<ZomeCallInvocation, SerializedBytesError>
where
    P: serde::Serialize + std::fmt::Debug,
{
    let ZomeCall {
        cell_id,
        cap_secret,
        fn_name,
        payload,
        provenance,
        signature,
        nonce,
        expires_at,
        ..
    } = new_zome_call(keystore, cell_id, func, payload, zome.clone().into()).await?;
    Ok(ZomeCallInvocation {
        cell_id,
        zome: zome.into(),
        cap_secret,
        fn_name,
        payload,
        provenance,
        signature,
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
pub async fn fake_genesis(
    vault: DbWrite<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    keystore: MetaLairClient,
) -> SourceChainResult<()> {
    fake_genesis_for_agent(vault, dht_db, fake_agent_pubkey_1(), keystore).await
}

/// Run genesis on the source chain for a specific agent for testing.
pub async fn fake_genesis_for_agent(
    vault: DbWrite<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    agent: AgentPubKey,
    keystore: MetaLairClient,
) -> SourceChainResult<()> {
    let dna = fake_dna_file("cool dna");
    let dna_hash = dna.dna_hash().clone();

    source_chain::genesis(
        vault,
        dht_db.clone(),
        &DhtDbQueryCache::new(dht_db.clone().into()),
        keystore,
        dna_hash,
        agent,
        None,
        None,
    )
    .await
}

/// Force all dht ops without enough validation receipts to be published.
pub async fn force_publish_dht_ops(
    vault: &DbWrite<DbKindAuthored>,
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
    publish_trigger.trigger(&"force_publish_dht_ops");
    Ok(())
}
