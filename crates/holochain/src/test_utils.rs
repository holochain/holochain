//! Utils for Holochain tests
use crate::conductor::api::AppInterfaceApi;
use crate::conductor::config::AdminInterfaceConfig;
use crate::conductor::config::ConductorConfig;
use crate::conductor::config::InterfaceDriver;
use crate::conductor::integration_dump;
use crate::conductor::p2p_agent_store;
use crate::conductor::ConductorBuilder;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::ZomeCallInvocation;
use ::fixt::prelude::*;
use aitia::Fact;
use hc_sleuth::SleuthId;
use hdk::prelude::ZomeName;
use holo_hash::fixt::*;
use holo_hash::*;
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_conductor_api::IntegrationStateDump;
use holochain_conductor_api::IntegrationStateDumps;
use holochain_conductor_api::ZomeCall;
use holochain_keystore::MetaLairClient;
use holochain_nonce::fresh_nonce;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_p2p::dht::prelude::Topology;
use holochain_p2p::dht::ArqStrat;
use holochain_p2p::dht::PeerViewQ;
use holochain_p2p::event::HolochainP2pEvent;
use holochain_p2p::spawn_holochain_p2p;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pRef;
use holochain_p2p::HolochainP2pSender;
use holochain_p2p::NetworkCompatParams;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::prelude::test_db_dir;
use holochain_state::prelude::SourceChainResult;
use holochain_state::prelude::StateQueryResult;
use holochain_state::source_chain;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::prelude::*;
use holochain_types::test_utils::fake_dna_file;
use holochain_types::test_utils::fake_dna_zomes;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p_types::config::KitsuneP2pConfig;
use kitsune_p2p_types::ok_fut;
use rusqlite::named_params;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::mpsc;

pub use itertools;

pub mod consistency;
pub mod hc_stress_test;
pub mod host_fn_caller;
pub mod inline_zomes;
pub mod network_simulation;

mod wait_for;
pub use wait_for::*;

mod big_stack_test;

mod generate_records;
pub use generate_records::*;
use holochain_types::websocket::AllowedOrigins;

use self::consistency::request_published_ops;

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
    let mut config = holochain_p2p::kitsune_p2p::dependencies::kitsune_p2p_types::config::KitsuneP2pConfig::default();
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
        NetworkCompatParams::default(),
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
                    respond.r(ok_fut(Ok(vec![])));
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
    agent: AgentPubKey,
    data: &[(DnaFile, Option<MembraneProof>)],
    conductor_handle: ConductorHandle,
) {
    for (dna, _) in data.iter() {
        conductor_handle.register_dna(dna.clone()).await.unwrap();
    }
    conductor_handle
        .clone()
        .install_app_minimal(name.to_string(), agent, data)
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
pub type DnasWithProofs = Vec<(DnaFile, Option<MembraneProof>)>;

/// One of various ways to setup an app, used somewhere...
pub async fn setup_app_in_new_conductor(
    installed_app_id: InstalledAppId,
    agent: AgentPubKey,
    dnas: DnasWithProofs,
) -> (Arc<TempDir>, AppInterfaceApi, ConductorHandle) {
    let db_dir = test_db_dir();

    let conductor_handle = ConductorBuilder::new()
        .with_data_root_path(db_dir.path().to_path_buf().into())
        .test(&[])
        .await
        .unwrap();

    install_app_in_conductor(conductor_handle.clone(), installed_app_id, agent, &dnas).await;

    let handle = conductor_handle.clone();

    (
        Arc::new(db_dir),
        AppInterfaceApi::new(conductor_handle),
        handle,
    )
}

/// Install an app into an existing conductor instance
pub async fn install_app_in_conductor(
    conductor_handle: ConductorHandle,
    installed_app_id: InstalledAppId,
    agent: AgentPubKey,
    dnas_with_proofs: &[(DnaFile, Option<MembraneProof>)],
) {
    for (dna, _) in dnas_with_proofs {
        conductor_handle.register_dna(dna.clone()).await.unwrap();
    }

    conductor_handle
        .clone()
        .install_app_minimal(installed_app_id.clone(), agent, dnas_with_proofs)
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
    network: KitsuneP2pConfig,
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
    network: Option<KitsuneP2pConfig>,
) -> (AppInterfaceApi, ConductorHandle) {
    let config = ConductorConfig {
        data_root_path: Some(data_root_path.clone()),
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket {
                port: 0,
                allowed_origins: AllowedOrigins::Any,
            },
        }]),
        network: network.unwrap_or_default(),
        ..Default::default()
    };
    let conductor_handle = ConductorBuilder::new()
        .config(config)
        .test(&[])
        .await
        .unwrap();

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

/// If HC_WASM_CACHE_PATH is set warm the cache
pub fn warm_wasm_tests() {
    if let Some(_path) = std::env::var_os("HC_WASM_CACHE_PATH") {
        let wasms: Vec<_> = TestWasm::iter().collect();
        crate::fixt::RealRibosomeFixturator::new(crate::fixt::curve::Zomes(wasms))
            .next()
            .unwrap();
    }
}

/// Consistency was failed to be reached. Here's a report.
#[derive(derive_more::From)]
pub struct ConsistencyError(String);

impl std::fmt::Debug for ConsistencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Alias
pub type ConsistencyResult = Result<(), ConsistencyError>;

/// Wait for all cell envs to reach consistency, meaning that every op
/// published by every cell has been integrated by every node
pub async fn consistency_dbs<AuthorDb, DhtDb>(
    all_cell_dbs: &[(&SleuthId, &AgentPubKey, &AuthorDb, Option<&DhtDb>)],
    timeout: Duration,
) -> ConsistencyResult
where
    AuthorDb: ReadAccess<DbKindAuthored>,
    DhtDb: ReadAccess<DbKindDht>,
{
    let mut published = HashSet::new();
    for (_, author, db, _) in all_cell_dbs.iter() {
        published.extend(
            request_published_ops(*db, Some((*author).to_owned()))
                .await
                .unwrap()
                .into_iter()
                .map(|(_, _, op)| op),
        );
    }
    let published = Arc::new(published.into_iter().collect::<Vec<_>>());
    let all_node_ids: HashSet<_> = all_cell_dbs
        .iter()
        .map(|(node_id, _, _, _)| node_id)
        .collect();

    futures::future::join_all(
        all_cell_dbs
            .iter()
            .flat_map(|(node_id, _, _, d)| Some((d.as_ref()?, node_id)))
            .map(move |(&db, node_id)| {
                let others: Vec<String> = all_node_ids
                    .difference(&[node_id].into_iter().collect())
                    .map(|n| n.to_string())
                    .collect();
                wait_for_integration_diff(others, db.clone(), published.clone(), timeout)
            }),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<()>, ConsistencyError>>()?;
    Ok(())
}

const CONSISTENCY_DELAY_LOW: Duration = Duration::from_millis(100);
const CONSISTENCY_DELAY_MID: Duration = Duration::from_millis(500);
const CONSISTENCY_DELAY_HIGH: Duration = Duration::from_millis(1000);

/// Wait for num_attempts * delay, or until all published ops have been integrated.
/// If the timeout is reached, print a report including a diff of all published ops
/// which were not integrated.
#[tracing::instrument(skip(db, published))]
async fn wait_for_integration_diff<Db: ReadAccess<DbKindDht>>(
    node_ids: Vec<SleuthId>,
    db: Db,
    published: Arc<Vec<DhtOp>>,
    timeout: Duration,
) -> ConsistencyResult {
    fn display_op(op: &DhtOp) -> String {
        match op {
            DhtOp::ChainOp(op) => format!(
                "{} {:>3}  {} ({})",
                op.action().author(),
                op.action().action_seq(),
                op.get_type(),
                op.action().action_type(),
            ),
            DhtOp::WarrantOp(op) => {
                format!("{} WARRANT ({})", op.author, op.get_type(),)
            }
        }
    }

    let header = format!("{:54} {:>3}  {}", "author", "seq", "op_type (action_type)",);
    let start = tokio::time::Instant::now();

    let num_published = published.len();
    while start.elapsed() < timeout {
        let num_integrated = get_integrated_count(&db).await;
        let delay = if num_integrated >= num_published {
            if num_integrated > num_published {
                tracing::warn!("num integrated ops ({}) > num published ops ({}), meaning you may not be accounting for all nodes in this test.
                Consistency may not be complete.", num_integrated, num_published)
            }
            return Ok(());
        } else {
            let total_time_waited = start.elapsed();
            let queries = query_integration(&db).await;
            tracing::debug!(?num_integrated, ?total_time_waited, counts = ?queries, "consistency-status");

            if total_time_waited > Duration::from_secs(10) {
                CONSISTENCY_DELAY_HIGH
            } else if total_time_waited > Duration::from_secs(1) {
                CONSISTENCY_DELAY_MID
            } else {
                CONSISTENCY_DELAY_LOW
            }
        };
        tokio::time::sleep(delay).await;
    }

    // Timeout has been reached at this point, so print a helpful report

    if published.is_empty() {
        return Err(format!("No ops were published in {timeout:?}").into());
    }

    // Otherwise just print a report of which ops were not integrated
    let mut published_displays: Vec<_> = published.iter().map(display_op).collect();
    let mut integrated: Vec<_> = get_integrated_ops(&db)
        .await
        .iter()
        .map(display_op)
        .collect();
    published_displays.sort();
    integrated.sort();

    let unintegrated = diff::slice(&published_displays, &integrated)
        .into_iter()
        .filter_map(|d| match d {
            diff::Result::Left(l) => Some(l),
            _ => None,
        })
        .cloned()
        .collect::<Vec<_>>();

    if unintegrated.is_empty() {
        // Even though the main loop failed, the final check shows that we have all ops!
        return Ok(());
    }

    if let Some(s) = hc_sleuth::SUBSCRIBER.get() {
        // If hc_sleuth has been initialized, print a sleuthy report

        let ctx = s.lock();
        for fact in published
            .iter()
            .map(DhtOpHash::with_data_sync)
            .flat_map(|hash| {
                node_ids
                    .iter()
                    .map(move |node_id| hc_sleuth::Event::Integrated {
                        by: node_id.clone(),
                        op: hash.clone(),
                    })
            })
        {
            let tr = fact.clone().traverse(&ctx);
            if let Some(report) = aitia::simple_report(&tr) {
                println!("aitia report for {fact:#?}:\n\n{report}")
            }
        }
    }

    let integration_dump = integration_dump(&db).await.unwrap();

    Err(format!(
        "Consistency not achieved after {:?}. Expected {} ops, but only {} integrated. Unintegrated ops:\n\n{}\n{}\n\n{:?}",
        timeout,
        num_published,
        integrated.len(),
        header,
        unintegrated.join("\n"),
        integration_dump,
    ).into())
}

/// Wait for num_attempts * delay, or until all published ops have been integrated.
#[tracing::instrument(skip(db))]
pub async fn wait_for_integration<Db: ReadAccess<DbKindDht>>(
    db: &Db,
    num_published: usize,
    num_attempts: usize,
    delay: Duration,
) {
    for i in 0..num_attempts {
        let num_integrated = get_integrated_count(db).await;
        if num_integrated >= num_published {
            if num_integrated > num_published {
                tracing::warn!("num integrated ops > num published ops, meaning you may not be accounting for all nodes in this test.
                Consistency may not be complete.")
            }
            return;
        } else {
            let total_time_waited = delay * i as u32;
            tracing::debug!(?num_integrated, ?total_time_waited, counts = ?query_integration(db).await);
        }
        tokio::time::sleep(delay).await;
    }

    panic!("Consistency not achieved after {} attempts", num_attempts);
}

#[tracing::instrument(skip(envs))]
/// Show authored data for each cell environment
pub async fn show_authored<Db: ReadAccess<DbKindAuthored>>(envs: &[&Db]) {
    for (i, &db) in envs.iter().enumerate() {
        db.read_async(move |txn| -> DatabaseResult<()> {
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

            Ok(())
        }).await.unwrap();
    }
}

/// Get multiple db states with compact Display representation
pub async fn get_integration_dumps<Db: ReadAccess<DbKindDht>>(
    dbs: &[&Db],
) -> IntegrationStateDumps {
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

async fn get_integrated_count<Db: ReadAccess<DbKindDht>>(db: &Db) -> usize {
    db.read_async(move |txn| -> DatabaseResult<usize> {
        Ok(txn.query_row(
            "SELECT COUNT(hash) FROM DhtOp WHERE DhtOp.when_integrated IS NOT NULL",
            [],
            |row| row.get(0),
        )?)
    })
    .await
    .unwrap()
}

/// Get all [`DhtOps`](holochain_types::prelude::DhtOp) integrated by this node
pub async fn get_integrated_ops<Db: ReadAccess<DbKindDht>>(db: &Db) -> Vec<DhtOp> {
    db.read_async(move |txn| -> StateQueryResult<Vec<DhtOp>> {
        txn.prepare(
            "
            SELECT
            DhtOp.type, Action.author as author, Action.blob as action_blob, Entry.blob as entry_blob
            FROM DhtOp
            JOIN
            Action ON DhtOp.action_hash = Action.hash
            LEFT JOIN
            Entry ON Action.entry_hash = Entry.hash
            WHERE
            DhtOp.when_integrated IS NOT NULL
            ORDER BY DhtOp.rowid ASC
        ",
        )
        .unwrap()
        .query_and_then(named_params! {}, |row| {
            Ok(holochain_state::query::map_sql_dht_op(true, "type", row).unwrap())
        })
        .unwrap()
        .collect::<StateQueryResult<_>>()
    })
    .await
    .unwrap()
}

/// Helper for displaying agent infos stored on a conductor
pub async fn display_agent_infos(conductor: &ConductorHandle) {
    for cell_id in conductor.running_cell_ids() {
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
        .write_async(|txn| {
            DatabaseResult::Ok(txn.execute(
                "UPDATE DhtOp SET last_publish_time = NULL WHERE receipts_complete IS NULL",
                [],
            )?)
        })
        .await?;
    publish_trigger.trigger(&"force_publish_dht_ops");
    Ok(())
}
