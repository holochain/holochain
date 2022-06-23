//! Types to help with building simulated networks.
//! Note this is an experimental prototype.
#![warn(missing_docs)]

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use ::fixt::prelude::*;
use hdk::prelude::*;
use holo_hash::{DhtOpHash, DnaHash};
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_p2p::dht_arc::{DhtArc, DhtArcRange, DhtLocation};
use holochain_p2p::{AgentPubKeyExt, DhtOpHashExt, DnaHashExt};
use holochain_sqlite::db::{p2p_put_single, AsP2pStateTxExt};
use holochain_state::prelude::from_blob;
use holochain_state::test_utils::fresh_reader_test;
use holochain_types::dht_op::{DhtOp, DhtOpHashed, DhtOpType};
use holochain_types::inline_zome::{InlineEntryTypes, InlineZomeSet};
use holochain_types::prelude::DnaFile;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::KitsuneP2pConfig;
use kitsune_p2p::{fixt::*, KitsuneAgent, KitsuneOpHash};
use rand::distributions::Alphanumeric;
use rand::distributions::Standard;
use rand::Rng;
use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::conductor::handle::DevSettingsDelta;
use crate::sweettest::{SweetConductor, SweetDnaFile};

#[derive(SerializedBytes, serde::Serialize, serde::Deserialize, Debug)]
/// Data to use to simulate a dht network.
pub struct MockNetworkData {
    /// The hashes authored by each agent.
    pub authored: HashMap<Arc<AgentPubKey>, Vec<Arc<DhtOpHash>>>,
    /// DhtOpHash -> KitsuneOpHash
    pub op_hash_to_kit: HashMap<Arc<DhtOpHash>, Arc<KitsuneOpHash>>,
    /// KitsuneOpHash -> DhtOpHash
    pub op_kit_to_hash: HashMap<Arc<KitsuneOpHash>, Arc<DhtOpHash>>,
    /// AgentPubKey -> KitsuneAgent
    pub agent_hash_to_kit: HashMap<Arc<AgentPubKey>, Arc<KitsuneAgent>>,
    /// KitsuneAgent -> AgentPubKey
    pub agent_kit_to_hash: HashMap<Arc<KitsuneAgent>, Arc<AgentPubKey>>,
    /// Agent storage arcs.
    pub agent_to_arc: HashMap<Arc<AgentPubKey>, DhtArc>,
    /// Agents peer info.
    pub agent_to_info: HashMap<Arc<AgentPubKey>, AgentInfoSigned>,
    /// Hashes ordered by their basis location.
    pub ops_by_loc: BTreeMap<DhtLocation, Vec<Arc<DhtOpHash>>>,
    /// Hash to basis location.
    pub op_to_loc: HashMap<Arc<DhtOpHash>, DhtLocation>,
    /// The DhtOps
    pub ops: HashMap<Arc<DhtOpHash>, DhtOpHashed>,
    /// The uuid for the integrity zome (also for the dna).
    pub integrity_uuid: String,
    /// The uuid for the coordinator zome.
    pub coordinator_uuid: String,
}

struct GeneratedData {
    integrity_uuid: String,
    coordinator_uuid: String,
    peer_data: Vec<AgentInfoSigned>,
    authored: HashMap<Arc<AgentPubKey>, Vec<Arc<DhtOpHash>>>,
    ops: HashMap<Arc<DhtOpHash>, DhtOpHashed>,
}

impl MockNetworkData {
    fn new(data: GeneratedData) -> Self {
        let GeneratedData {
            integrity_uuid,
            coordinator_uuid,
            peer_data,
            authored,
            ops,
        } = data;
        let (agent_hash_to_kit, agent_kit_to_hash): (HashMap<_, _>, HashMap<_, _>) = authored
            .keys()
            .map(|agent| {
                let k_agent = agent.to_kitsune();
                ((agent.clone(), k_agent.clone()), (k_agent, agent.clone()))
            })
            .unzip();
        let mut op_hash_to_kit = HashMap::with_capacity(ops.len());
        let mut op_kit_to_hash = HashMap::with_capacity(ops.len());
        let mut ops_by_loc = BTreeMap::new();
        let mut op_to_loc = HashMap::with_capacity(ops.len());
        for (hash, op) in &ops {
            let k_hash = hash.to_kitsune();
            op_hash_to_kit.insert(hash.clone(), k_hash.clone());
            op_kit_to_hash.insert(k_hash, hash.clone());

            let loc = op.dht_basis().get_loc();

            ops_by_loc
                .entry(loc)
                .or_insert_with(Vec::new)
                .push(hash.clone());
            op_to_loc.insert(hash.clone(), loc);
        }
        let agent_to_info: HashMap<_, _> = peer_data
            .into_iter()
            .map(|info| (agent_kit_to_hash[&info.agent].clone(), info))
            .collect();
        let agent_to_arc = agent_to_info
            .iter()
            .map(|(k, v)| (k.clone(), v.storage_arc))
            .collect();
        Self {
            authored,
            op_hash_to_kit,
            op_kit_to_hash,
            agent_hash_to_kit,
            agent_kit_to_hash,
            agent_to_arc,
            agent_to_info,
            ops_by_loc,
            op_to_loc,
            ops,
            integrity_uuid,
            coordinator_uuid,
        }
    }

    /// Number of agents in this simulation.
    /// This doesn't include any real agents.
    pub fn num_agents(&self) -> usize {
        self.agent_to_info.len()
    }

    /// The simulated agents.
    pub fn agents(&self) -> impl Iterator<Item = &Arc<AgentPubKey>> {
        self.agent_to_info.keys()
    }

    /// The coverage of the simulated dht.
    pub fn coverage(&self) -> f64 {
        ((50.0 / self.num_agents() as f64) * 2.0).clamp(0.0, 1.0)
    }

    /// The agent info of the simulated agents.
    pub fn agent_info(&self) -> impl Iterator<Item = &AgentInfoSigned> {
        self.agent_to_info.values()
    }

    /// Hashes that an agent is an authority for.
    pub fn hashes_authority_for(&self, agent: &AgentPubKey) -> Vec<Arc<DhtOpHash>> {
        let arc = self.agent_to_arc[agent].interval();
        match arc {
            DhtArcRange::Empty => Vec::with_capacity(0),
            DhtArcRange::Full => self.ops_by_loc.values().flatten().cloned().collect(),
            DhtArcRange::Bounded(start, end) => {
                if start <= end {
                    self.ops_by_loc
                        .range(start..=end)
                        .flat_map(|(_, hash)| hash)
                        .cloned()
                        .collect()
                } else {
                    self.ops_by_loc
                        .range(..=end)
                        .flat_map(|(_, hash)| hash)
                        .chain(self.ops_by_loc.range(start..).flat_map(|(_, hash)| hash))
                        .cloned()
                        .collect()
                }
            }
        }
    }
}

/// Generate test data for a simulated network using holochain.
/// The data can be cached to the tmp directory
/// which can save time on running tests or it can
/// be all held in memory.
pub async fn generate_test_data(
    num_agents: usize,
    min_num_ops_held: usize,
    in_memory: bool,
    force_new_data: bool,
) -> (MockNetworkData, Connection) {
    let cached = if !in_memory || !force_new_data {
        get_cached().filter(|data| data.authored.len() == num_agents)
    } else {
        None
    };
    let is_cached = cached.is_some();
    let (data, dna_hash) = match cached {
        Some(cached) => {
            let dna_file = data_zome(
                cached.integrity_uuid.clone(),
                cached.coordinator_uuid.clone(),
            )
            .await;
            let dna_hash = dna_file.dna_hash().clone();
            (cached, dna_hash)
        }
        None => {
            let integrity_uuid = nanoid::nanoid!();
            let coordinator_uuid = nanoid::nanoid!();

            let dna_file = data_zome(integrity_uuid.clone(), coordinator_uuid.clone()).await;
            let dna_hash = dna_file.dna_hash().clone();
            let data = create_test_data(
                num_agents,
                min_num_ops_held,
                dna_file,
                integrity_uuid,
                coordinator_uuid,
            )
            .await;
            (data, dna_hash)
        }
    };
    let generated_data = GeneratedData {
        ops: data.ops,
        peer_data: reset_peer_data(data.peer_data, &dna_hash).await,
        integrity_uuid: data.integrity_uuid,
        coordinator_uuid: data.coordinator_uuid,
        authored: data.authored,
    };
    let data = MockNetworkData::new(generated_data);
    let conn = cache_data(in_memory, &data, is_cached);
    (data, conn)
}

fn cache_data(in_memory: bool, data: &MockNetworkData, is_cached: bool) -> Connection {
    let mut conn = if in_memory {
        Connection::open_in_memory().unwrap()
    } else {
        let p = std::env::temp_dir().join("mock_test_data");
        std::fs::create_dir(&p).ok();
        let p = p.join("mock_test_data.sqlite3");
        Connection::open(p).unwrap()
    };
    if is_cached && !in_memory {
        return conn;
    }
    holochain_sqlite::schema::SCHEMA_CELL
        .initialize(&mut conn, None)
        .unwrap();
    holochain_sqlite::schema::SCHEMA_P2P_STATE
        .initialize(&mut conn, None)
        .unwrap();
    let mut txn = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
        .unwrap();
    txn.execute(
        "
        CREATE TABLE IF NOT EXISTS Authored (
            agent       BLOB    NOT NULL,
            dht_op_hash BLOB    NOT NUll
        )
        ",
        [],
    )
    .unwrap();
    txn.execute(
        "
        CREATE TABLE IF NOT EXISTS Uuid(
            integrity_uuid TEXT NOT NULL,
            coordinator_uuid TEXT NOT NULL
        )
        ",
        [],
    )
    .unwrap();
    txn.execute(
        "
        INSERT INTO Uuid (integrity_uuid, coordinator_uuid) VALUES(?)
        ",
        [&data.integrity_uuid, &data.coordinator_uuid],
    )
    .unwrap();
    for op in data.ops.values() {
        holochain_state::test_utils::mutations_helpers::insert_valid_integrated_op(&mut txn, op)
            .unwrap();
    }
    for (author, ops) in &data.authored {
        for op in ops {
            txn.execute(
                "
                    INSERT INTO Authored (agent, dht_op_hash)
                    VALUES(?, ?)
                    ",
                params![author, op.as_hash()],
            )
            .unwrap();
        }
    }
    for agent in data.agent_to_info.values() {
        p2p_put_single(&mut txn, agent).unwrap();
    }
    txn.commit().unwrap();
    conn
}

fn get_cached() -> Option<GeneratedData> {
    let p = std::env::temp_dir()
        .join("mock_test_data")
        .join("mock_test_data.sqlite3");
    p.exists().then(|| ()).and_then(|_| {
        let mut conn = Connection::open(p).ok()?;
        let mut txn = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
            .unwrap();
        // If there's no uuid then there's no data.
        let integrity_uuid = txn
            .query_row("SELECT integrity_uuid FROM Uuid", [], |row| row.get(0))
            .optional()
            .ok()
            .flatten()?;
        let coordinator_uuid = txn
            .query_row("SELECT coordinator_uuid FROM Uuid", [], |row| row.get(0))
            .optional()
            .ok()
            .flatten()?;
        let ops = get_ops(&mut txn);
        let peer_data = txn.p2p_list_agents().unwrap();
        let authored = txn
            .prepare("SELECT agent, dht_op_hash FROM Authored")
            .unwrap()
            .query_map([], |row| Ok((Arc::new(row.get(0)?), Arc::new(row.get(1)?))))
            .unwrap()
            .map(Result::unwrap)
            .fold(HashMap::new(), |mut map, (agent, hash)| {
                map.entry(agent).or_insert_with(Vec::new).push(hash);
                map
            });

        Some(GeneratedData {
            integrity_uuid,
            coordinator_uuid,
            peer_data,
            authored,
            ops,
        })
    })
}

async fn create_test_data(
    num_agents: usize,
    approx_num_ops_held: usize,
    dna_file: DnaFile,
    integrity_uuid: String,
    coordinator_uuid: String,
) -> GeneratedData {
    let coverage = ((50.0 / num_agents as f64) * 2.0).clamp(0.0, 1.0);
    let num_storage_buckets = (1.0 / coverage).round() as u32;
    let bucket_size = u32::MAX / num_storage_buckets;
    let buckets = (0..num_storage_buckets)
        .map(|i| DhtArcRange::from_bounds(i * bucket_size, i * bucket_size + bucket_size))
        .collect::<Vec<_>>();
    let mut bucket_counts = vec![0; buckets.len()];
    let mut entries = Vec::with_capacity(buckets.len() * approx_num_ops_held);
    let rng = rand::thread_rng();
    let mut rand_entry = rng.sample_iter(&Standard);
    let rand_entry = rand_entry.by_ref();
    let start = std::time::Instant::now();
    loop {
        let d: Vec<u8> = rand_entry.take(10).collect();
        let d = UnsafeBytes::from(d);
        let entry = Entry::app(d.try_into().unwrap()).unwrap();
        let hash = EntryHash::with_data_sync(&entry);
        let loc = hash.get_loc();
        if let Some(index) = buckets.iter().position(|b| b.contains(&loc)) {
            if bucket_counts[index] < approx_num_ops_held * 100 {
                entries.push(entry);
                bucket_counts[index] += 1;
            }
        }
        if bucket_counts
            .iter()
            .all(|&c| c >= approx_num_ops_held * 100)
        {
            break;
        }
    }
    dbg!(bucket_counts);
    dbg!(start.elapsed());

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "none".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning);
    let config = ConductorConfig {
        network: Some(network),
        ..Default::default()
    };
    let mut conductor = SweetConductor::from_config(config).await;
    conductor.update_dev_settings(DevSettingsDelta {
        publish: Some(false),
        ..Default::default()
    });
    let mut agents = Vec::new();
    dbg!("generating agents");
    for i in 0..num_agents {
        eprintln!("generating agent {}", i);
        let agent = conductor
            .keystore()
            .clone()
            .new_sign_keypair_random()
            .await
            .unwrap();
        agents.push(agent);
    }

    dbg!("Installing apps");

    let apps = conductor
        .setup_app_for_agents("app", &agents, &[dna_file.clone()])
        .await
        .unwrap();

    let cells = apps.cells_flattened();
    let mut entries = entries.into_iter();
    let entries = entries.by_ref();
    for (i, cell) in cells.iter().enumerate() {
        eprintln!("Calling {}", i);
        let e = entries.take(approx_num_ops_held).collect::<Vec<_>>();
        let _: () = conductor.call(&cell.zome("zome1"), "create_many", e).await;
    }
    let mut authored = HashMap::new();
    let mut ops = HashMap::new();
    for (i, cell) in cells.iter().enumerate() {
        eprintln!("Extracting data {}", i);
        let db = cell.authored_db().clone();
        let data = fresh_reader_test(db, |mut txn| {
            get_authored_ops(&mut txn, cell.agent_pubkey())
        });
        let hashes = data.keys().cloned().collect::<Vec<_>>();
        authored.insert(Arc::new(cell.agent_pubkey().clone()), hashes);
        ops.extend(data);
    }
    dbg!("Getting agent info");
    let peer_data = conductor.get_agent_infos(None).await.unwrap();
    dbg!("Done");
    GeneratedData {
        integrity_uuid,
        coordinator_uuid,
        peer_data,
        authored,
        ops,
    }
}

/// Set the peers to seem like they come from separate nodes and have accurate storage arcs.
async fn reset_peer_data(peers: Vec<AgentInfoSigned>, dna_hash: &DnaHash) -> Vec<AgentInfoSigned> {
    let coverage = ((50.0 / peers.len() as f64) * 2.0).clamp(0.0, 1.0);
    let space_hash = dna_hash.to_kitsune();
    let mut peer_data = Vec::with_capacity(peers.len());
    let rng = rand::thread_rng();
    let mut rand_string = rng.sample_iter(&Alphanumeric);
    let rand_string = rand_string.by_ref();
    for peer in peers {
        let rand_string: String = rand_string.take(16).map(char::from).collect();
        let info = AgentInfoSigned::sign(
            space_hash.clone(),
            peer.agent.clone(),
            ((u32::MAX / 2) as f64 * coverage) as u32,
            vec![url2::url2!(
                "kitsune-proxy://CIW6PxKxs{}MSmB7kLD8xyyj4mqcw/kitsune-quic/h/localhost/p/5778/-",
                rand_string
            )
            .into()],
            peer.signed_at_ms,
            (std::time::UNIX_EPOCH.elapsed().unwrap() + std::time::Duration::from_secs(60_000_000))
                .as_millis() as u64,
            |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Predictable))) },
        )
        .await
        .unwrap();
        peer_data.push(info);
    }
    peer_data
}

fn get_ops(txn: &mut Transaction<'_>) -> HashMap<Arc<DhtOpHash>, DhtOpHashed> {
    txn.prepare(
        "
                SELECT DhtOp.hash, DhtOp.type AS dht_type,
                Action.blob AS action_blob, Entry.blob AS entry_blob
                FROM DHtOp
                JOIN Action ON DhtOp.action_hash = Action.hash
                LEFT JOIN Entry ON Action.entry_hash = Entry.hash
            ",
    )
    .unwrap()
    .query_map([], |row| {
        let action = from_blob::<SignedAction>(row.get("action_blob")?).unwrap();
        let op_type: DhtOpType = row.get("dht_type")?;
        let hash: DhtOpHash = row.get("hash")?;
        // Check the entry isn't private before gossiping it.
        let e: Option<Vec<u8>> = row.get("entry_blob")?;
        let entry = e.map(|entry| from_blob::<Entry>(entry).unwrap());
        let op = DhtOp::from_type(op_type, action, entry).unwrap();
        let op = DhtOpHashed::with_pre_hashed(op, hash.clone());
        Ok((Arc::new(hash), op))
    })
    .unwrap()
    .collect::<Result<HashMap<_, _>, _>>()
    .unwrap()
}

fn get_authored_ops(
    txn: &mut Transaction<'_>,
    author: &AgentPubKey,
) -> HashMap<Arc<DhtOpHash>, DhtOpHashed> {
    txn.prepare(
        "
                SELECT DhtOp.hash, DhtOp.type AS dht_type,
                Action.blob AS action_blob, Entry.blob AS entry_blob
                FROM DHtOp
                JOIN Action ON DhtOp.action_hash = Action.hash
                LEFT JOIN Entry ON Action.entry_hash = Entry.hash
                WHERE
                Action.author = ?
            ",
    )
    .unwrap()
    .query_map([author], |row| {
        let action = from_blob::<SignedAction>(row.get("action_blob")?).unwrap();
        let op_type: DhtOpType = row.get("dht_type")?;
        let hash: DhtOpHash = row.get("hash")?;
        // Check the entry isn't private before gossiping it.
        let e: Option<Vec<u8>> = row.get("entry_blob")?;
        let entry = e.map(|entry| from_blob::<Entry>(entry).unwrap());
        let op = DhtOp::from_type(op_type, action, entry).unwrap();
        let op = DhtOpHashed::with_pre_hashed(op, hash.clone());
        Ok((Arc::new(hash), op))
    })
    .unwrap()
    .collect::<Result<HashMap<_, _>, _>>()
    .unwrap()
}

/// The zome to use for this simulation.
/// Currently this is a limitation of this prototype that
/// you must use the data generation zome in the actual simulation
/// so the Dna record matches.
/// Hopefully this limitation can be overcome in the future.
pub async fn data_zome(integrity_uuid: String, coordinator_uuid: String) -> DnaFile {
    let integrity_zome_name = "integrity_zome1";
    let coordinator_zome_name = "zome1";

    let zomes = InlineZomeSet::new(
        [(
            integrity_zome_name,
            integrity_uuid.clone(),
            InlineEntryTypes::entry_defs(),
            0,
        )],
        [(coordinator_zome_name, coordinator_uuid)],
    )
    .callback(
        coordinator_zome_name,
        "create_many",
        move |api, entries: Vec<Entry>| {
            for entry in entries {
                api.create(CreateInput::new(
                    InlineZomeSet::get_entry_location(&api, InlineEntryTypes::A),
                    EntryVisibility::Public,
                    entry,
                    ChainTopOrdering::default(),
                ))?;
            }
            Ok(())
        },
    )
    .callback(coordinator_zome_name, "read", |api, hash: ActionHash| {
        api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
            .map(|e| e.into_iter().next().unwrap())
            .map_err(Into::into)
    });
    let (dna_file, _, _) = SweetDnaFile::from_inline_zomes(integrity_uuid, zomes)
        .await
        .unwrap();
    dna_file
}
