//! Utils for Holochain tests
use crate::conductor::api::AppInterfaceApi;
use crate::conductor::config::AdminInterfaceConfig;
use crate::conductor::config::ConductorConfig;
use crate::conductor::config::InterfaceDriver;
use crate::conductor::ConductorBuilder;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::ZomeCallInvocation;
use ::fixt::prelude::*;
use hdk::prelude::ZomeName;
use holo_hash::*;
use holochain_conductor_api::conductor::paths::DataRootPath;
use holochain_conductor_api::conductor::NetworkConfig;
use holochain_conductor_api::IntegrationStateDump;
use holochain_conductor_api::IntegrationStateDumps;
use holochain_conductor_api::ZomeCallParamsSigned;
use holochain_keystore::MetaLairClient;
use holochain_nonce::fresh_nonce;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::prelude::test_db_dir;
use holochain_state::prelude::SourceChainResult;
use holochain_state::prelude::StateQueryResult;
use holochain_state::source_chain;
use holochain_types::prelude::*;
use holochain_types::test_utils::fake_dna_file;
use holochain_types::test_utils::fake_dna_zomes;
use holochain_wasm_test_utils::TestWasm;
pub use itertools;
use rusqlite::named_params;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::error::Elapsed;

pub mod consistency;
pub mod hc_stress_test;
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
        .test(&[])
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
        crate::fixt::RealRibosomeFixturator::new(crate::fixt::Zomes(wasms))
            .next()
            .unwrap();
    }
}

/// Wait for num_attempts * delay, or until all published ops have been integrated.
#[cfg_attr(feature = "instrument", tracing::instrument(skip(db)))]
pub async fn wait_for_integration<Db: ReadAccess<DbKindDht>>(
    db: &Db,
    num_published: usize,
    num_attempts: usize,
    delay: Duration,
) -> Result<(), String> {
    let mut num_integrated = 0;
    for i in 0..num_attempts {
        num_integrated = get_integrated_count(db).await;
        if num_integrated >= num_published {
            if num_integrated > num_published {
                tracing::warn!("num integrated ops > num published ops, meaning you may not be accounting for all nodes in this test.
                Consistency may not be complete.")
            }
            return Ok(());
        } else {
            let total_time_waited = delay * i as u32;
            tracing::debug!(?num_integrated, ?total_time_waited, counts = ?query_integration(db).await, "consistency-status");
        }
        tokio::time::sleep(delay).await;
    }

    Err(format!(
        "Consistency not achieved after {num_attempts} attempts. Expected {num_published} ops, but only {num_integrated} integrated.",
    ))
}

/// Show authored data for each cell environment
#[cfg_attr(feature = "instrument", tracing::instrument(skip(envs)))]
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

/// Get count of ops that have been successfully validated but not integrated
pub async fn get_valid_and_not_integrated_count<Db: ReadAccess<DbKindDht>>(db: &Db) -> usize {
    db.read_async(move |txn| -> DatabaseResult<usize> {
        Ok(txn.query_row(
            "SELECT COUNT(hash) FROM DhtOp WHERE when_integrated IS NULL AND validation_status = :status",
            named_params!{
                ":status": ValidationStatus::Valid,
            },
            |row| row.get(0),
        )?)
    })
    .await
    .unwrap()
}

/// Get count of ops that have been successfully validated and integrated
pub async fn get_valid_and_integrated_count<Db: ReadAccess<DbKindDht>>(db: &Db) -> usize {
    db.read_async(move |txn| -> DatabaseResult<usize> {
        Ok(txn.query_row(
            "SELECT COUNT(hash) FROM DhtOp WHERE when_integrated IS NOT NULL AND validation_status = :status",
            named_params!{
                ":status": ValidationStatus::Valid,
            },
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
            DhtOp.type,
            Action.author as author,
            Action.blob as action_blob,
            Entry.blob as entry_blob
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
        payload,
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

    source_chain::genesis(vault, dht_db.clone(), keystore, dna_hash, agent, None, None).await
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

        let mut conductor = SweetConductor::from_standard_config().await;

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
