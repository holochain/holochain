//! Utilities for testing the consistency of the dht.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use futures::stream::StreamExt;
use holo_hash::{DhtOpHash, DnaHash};
use holochain_p2p::{dht_arc::DhtArc, AgentPubKeyExt, DhtOpHashExt, DnaHashExt};
use holochain_sqlite::{db::AsP2pStateTxExt, prelude::DatabaseResult};
use holochain_state::prelude::StateQueryResult;
use holochain_types::{dht_op::DhtOpType, env::EnvRead};
use kitsune_p2p::{KitsuneAgent, KitsuneBinType, KitsuneOpHash};
use kitsune_p2p_types::consistency::*;
use rusqlite::named_params;

use crate::conductor::ConductorHandle;

struct Stores {
    agent: Arc<KitsuneAgent>,
    cell_env: EnvRead,
    p2p_env: EnvRead,
}

#[derive(Clone)]
struct Reporter(tokio::sync::mpsc::Sender<SessionMessage>, Arc<KitsuneAgent>);

const CONCURRENCY: usize = 100;

/// A helper for checking consistency of all published ops for all cells in all conductors
/// has reached consistency in a sharded context.
pub async fn local_machine_session(conductors: &[ConductorHandle], timeout: Duration) {
    let mut spaces = HashMap::new();
    for (i, c) in conductors.iter().enumerate() {
        // TODO: Handle error.
        for cell_id in c.list_cell_ids(None).await.unwrap() {
            let space = spaces
                .entry(cell_id.dna_hash().clone())
                .or_insert_with(|| vec![None; conductors.len()]);
            if space[i].is_none() {
                let p2p_env: EnvRead = c.get_p2p_env(cell_id.dna_hash().to_kitsune()).await.into();
                space[i] = Some((p2p_env, Vec::new()));
            }
            space[i].as_mut().unwrap().1.push((
                // TODO: Handle error.
                c.get_cell_env_readonly(&cell_id).await.unwrap(),
                cell_id.agent_pubkey().to_kitsune(),
            ));
        }
    }
    for (_, conductors) in spaces {
        let mut wait_for_agents = HashSet::new();
        let (tx, rx) = tokio::sync::mpsc::channel(1000);
        for c in conductors {
            if let Some((p2p_env, agents)) = c {
                wait_for_agents.extend(agents.iter().map(|(_, agent)| agent.clone()));
                tokio::spawn(expect_all(tx.clone(), p2p_env, agents, timeout.clone()));
            }
        }
        wait_for_consistency(rx, wait_for_agents, timeout.clone()).await;
    }
}

/// Get consistency for a particular hash.
pub async fn local_machine_session_with_hashes(
    conductors: Vec<&ConductorHandle>,
    hashes: impl Iterator<Item = &DhtOpHash>,
    space: &DnaHash,
    timeout: Duration,
) {
    let mut spaces = HashMap::new();
    for (i, c) in conductors.iter().enumerate() {
        // TODO: Handle error.
        for cell_id in c.list_cell_ids(None).await.unwrap() {
            if cell_id.dna_hash() != space {
                continue;
            }
            let space = spaces
                .entry(cell_id.dna_hash().clone())
                .or_insert_with(|| vec![None; conductors.len()]);
            if space[i].is_none() {
                let p2p_env: EnvRead = c.get_p2p_env(cell_id.dna_hash().to_kitsune()).await.into();
                space[i] = Some((p2p_env, Vec::new()));
            }
            space[i].as_mut().unwrap().1.push((
                // TODO: Handle error.
                c.get_cell_env_readonly(&cell_id).await.unwrap(),
                cell_id.agent_pubkey().to_kitsune(),
            ));
        }
    }
    let all_hashes = hashes
        .into_iter()
        .map(|h| h.to_kitsune())
        .collect::<Vec<_>>();
    for (_, conductors) in spaces {
        let mut wait_for_agents = HashSet::new();
        let (tx, rx) = tokio::sync::mpsc::channel(1000);
        for c in conductors {
            if let Some((p2p_env, agents)) = c {
                wait_for_agents.extend(agents.iter().map(|(_, agent)| agent.clone()));
                tokio::spawn({
                    let all_hashes = all_hashes.clone();
                    let tx = tx.clone();
                    let agent_env_map: HashMap<_, _> =
                        agents.iter().cloned().map(|(e, a)| (a, e)).collect();
                    async move {
                        let mut all_agents = Vec::with_capacity(agents.len());
                        for (_, agent) in &agents {
                            // TODO: Handle error.
                            if let Some(storage_arc) = request_arc(&p2p_env, &agent).await.unwrap()
                            {
                                all_agents.push((agent.clone(), storage_arc));
                            }
                        }
                        let iter =
                            generate_session(&all_agents, &all_hashes, timeout, agent_env_map);
                        check_all(iter, tx, p2p_env).await;
                    }
                });
            }
        }
        wait_for_consistency(rx, wait_for_agents, timeout.clone()).await;
    }
}

impl Reporter {
    async fn send_report(&self, report: SessionReport) {
        if self
            .0
            .send(SessionMessage {
                from: self.1.clone(),
                report,
            })
            .await
            .is_err()
        {
            tracing::error!("Failed to message for consistency session");
        }
    }
}

async fn wait_for_consistency(
    mut rx: tokio::sync::mpsc::Receiver<SessionMessage>,
    mut agents: HashSet<Arc<KitsuneAgent>>,
    timeout: Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout + Duration::from_millis(100);
    while let Ok(Some(SessionMessage { from, report })) =
        tokio::time::timeout_at(deadline, rx.recv()).await
    {
        match report {
            SessionReport::KeepAlive {
                missing_agents,
                missing_hashes,
            } => {
                tracing::debug!(
                    "{:?} is still missing {} agents and {} hashes",
                    from,
                    missing_agents,
                    missing_hashes
                );
            }
            SessionReport::Complete { elapsed_ms } => {
                agents.remove(&from);
                tracing::debug!("{:?} has reached consistency in {}ms", from, elapsed_ms);
                if agents.is_empty() {
                    return;
                }
            }
            SessionReport::Timeout {
                missing_agents,
                missing_hashes,
            } => {
                agents.remove(&from);
                tracing::debug!(
                    "{:?} has timed out before reaching consistency. \nMissing Agents: {:?}\nMissing Hashes: {:?}", 
                    from,
                    missing_agents,
                    missing_hashes
                );
                if agents.is_empty() {
                    return;
                }
            }
            SessionReport::Error { error } => {
                agents.remove(&from);
                tracing::debug!(
                    "{:?} has failed the consistency session with error {}",
                    from,
                    error
                );
                if agents.is_empty() {
                    return;
                }
            }
        }
    }
}

async fn expect_all(
    tx: tokio::sync::mpsc::Sender<SessionMessage>,
    p2p_env: EnvRead,
    agents: Vec<(EnvRead, Arc<KitsuneAgent>)>,
    timeout: Duration,
) {
    let stores = agents.iter().cloned().map(|(cell_env, agent)| Stores {
        agent,
        cell_env,
        p2p_env: p2p_env.clone(),
    });
    let agent_env_map: HashMap<_, _> = agents.iter().cloned().map(|(e, a)| (a, e)).collect();
    // TODO: Handle error.
    let all_published_data = gather_published_data(stores, CONCURRENCY).await.unwrap();
    let mut all_hashes = Vec::new();
    let mut all_agents = Vec::with_capacity(all_published_data.len());
    for PublishedData {
        agent,
        storage_arc,
        published_hashes,
    } in all_published_data
    {
        all_hashes.extend(published_hashes);
        all_agents.push((agent, storage_arc));
    }

    let iter = generate_session(&all_agents, &all_hashes, timeout, agent_env_map);
    check_all(iter, tx, p2p_env).await;
}

async fn check_all(
    iter: impl Iterator<Item = (Arc<KitsuneAgent>, ConsistencySession, EnvRead)>,
    tx: tokio::sync::mpsc::Sender<SessionMessage>,
    p2p_env: EnvRead,
) {
    futures::stream::iter(iter)
        .for_each_concurrent(CONCURRENCY, |(agent, expected_session, cell_env)| {
            let tx = tx.clone();
            let p2p_env = p2p_env.clone();
            let reporter = Reporter(tx.clone(), agent);
            check_expected_data(reporter, expected_session, cell_env, p2p_env)
        })
        .await;
}
async fn check_expected_data(
    reporter: Reporter,
    session: ConsistencySession,
    vault: EnvRead,
    p2p_env: EnvRead,
) {
    if let Err(e) = check_expected_data_inner(reporter.clone(), session, vault, p2p_env).await {
        reporter
            .send_report(SessionReport::Error {
                error: e.to_string(),
            })
            .await;
    }
}

fn generate_session<'iter>(
    all_agents: &'iter Vec<(Arc<KitsuneAgent>, DhtArc)>,
    all_hashes: &'iter Vec<Arc<KitsuneOpHash>>,
    timeout: Duration,
    agent_env_map: HashMap<Arc<KitsuneAgent>, EnvRead>,
) -> impl Iterator<Item = (Arc<KitsuneAgent>, ConsistencySession, EnvRead)> + 'iter {
    all_agents
        .iter()
        .map(move |(agent, arc)| {
            let mut published_hashes = Vec::new();
            let mut expected_agents = Vec::new();
            for hash in all_hashes.iter() {
                if arc.contains(hash.get_loc()) {
                    published_hashes.push(hash.clone());
                }
            }
            for (agent, _) in all_agents.iter() {
                if arc.contains(kitsune_p2p::KitsuneBinType::get_loc(&(**agent))) {
                    expected_agents.push(agent.clone());
                }
            }
            (
                agent.clone(),
                ExpectedData {
                    expected_agents,
                    expected_hashes: published_hashes,
                },
            )
        })
        .map(move |(agent, expected_data)| {
            (
                agent,
                ConsistencySession {
                    keep_alive: Some(Duration::from_secs(1)),
                    frequency: Duration::from_millis(100),
                    timeout,
                    expected_data,
                },
            )
        })
        .filter_map(move |(agent, expected_session)| {
            agent_env_map
                .get(&agent)
                .cloned()
                .map(|env| (agent, expected_session, env))
        })
}
async fn check_expected_data_inner(
    reporter: Reporter,
    session: ConsistencySession,
    vault: EnvRead,
    p2p_env: EnvRead,
) -> DatabaseResult<()> {
    let ConsistencySession {
        keep_alive,
        frequency,
        timeout,
        expected_data:
            ExpectedData {
                expected_agents,
                mut expected_hashes,
            },
    } = session;

    let start = tokio::time::Instant::now();
    let mut last_keep_alive = start;
    let deadline = tokio::time::Instant::now() + timeout;
    let mut frequency = tokio::time::interval(frequency);
    let mut missing_agents = Vec::with_capacity(expected_agents.len());
    let mut missing_hashes = Vec::with_capacity(expected_hashes.len());
    while let Ok(_) = tokio::time::timeout_at(deadline, frequency.tick()).await {
        missing_agents = check_agents(&p2p_env, &expected_agents).await?.collect();

        check_hashes(&vault, &mut expected_hashes, &mut missing_hashes).await?;
        if missing_agents.is_empty() && missing_hashes.is_empty() {
            reporter
                .send_report(SessionReport::Complete {
                    elapsed_ms: start.elapsed().as_millis() as u32,
                })
                .await;
            return Ok(());
        }
        if keep_alive.map_or(false, |k| last_keep_alive.elapsed() > k) {
            reporter
                .send_report(SessionReport::KeepAlive {
                    missing_agents: missing_agents.len() as u32,
                    missing_hashes: missing_hashes.len() as u32,
                })
                .await;
            last_keep_alive = tokio::time::Instant::now();
        }
    }
    reporter
        .send_report(SessionReport::Timeout {
            missing_agents: missing_agents.into_iter().cloned().collect(),
            missing_hashes,
        })
        .await;
    Ok(())
}

async fn check_agents<'iter>(
    p2p_env: &EnvRead,
    expected_agents: &'iter [Arc<KitsuneAgent>],
) -> DatabaseResult<impl Iterator<Item = &'iter Arc<KitsuneAgent>>> {
    let agents_held: HashSet<_> = p2p_env
        .async_reader(|txn| {
            DatabaseResult::Ok(
                txn.p2p_list_agents()?
                    .into_iter()
                    .map(|a| a.agent.clone())
                    .collect(),
            )
        })
        .await?;
    Ok(expected_agents
        .iter()
        .filter(move |a| !agents_held.contains(&(**a))))
}

async fn check_hashes(
    vault: &EnvRead,
    expected_hashes: &mut Vec<Arc<KitsuneOpHash>>,
    missing_hashes: &mut Vec<Arc<KitsuneOpHash>>,
) -> DatabaseResult<()> {
    missing_hashes.clear();
    // We need to swap these hashes so we can move them into the async_reader
    // without reallocating.
    let expected = std::mem::replace(expected_hashes, Vec::with_capacity(0));
    let mut missing = std::mem::replace(missing_hashes, Vec::with_capacity(0));
    let mut r = vault
                .async_reader(move |txn| {
                    for hash in &expected {
                        // TODO: This might be too slow, could instead save the holochain hash versions.
                        let h_hash: DhtOpHash = DhtOpHashExt::from_kitsune(&hash);
                        let integrated: bool = txn.query_row(
                            "
                            SELECT EXISTS(
                                SELECT 1 FROM DhtOp WHERE hash = :hash AND when_integrated IS NOT NULL
                            )
                            ",
                            named_params! {
                                ":hash": h_hash,
                            },
                            |row| row.get(0),
                        )?;
                        if !integrated {
                            missing.push(hash.clone());
                        }
                    }
                    DatabaseResult::Ok((expected, missing))
                })
                .await?;
    // Put the data back.
    std::mem::swap(&mut r.0, expected_hashes);
    std::mem::swap(&mut r.1, missing_hashes);
    Ok(())
}

async fn gather_published_data(
    iter: impl Iterator<Item = Stores>,
    concurrency: usize,
) -> StateQueryResult<Vec<PublishedData>> {
    use futures::stream::TryStreamExt;
    let iter = iter.map(|stores| async move {
        let published_hashes = request_published_ops(&stores.cell_env).await?;
        let storage_arc = request_arc(&stores.p2p_env, &stores.agent).await?;
        Ok(storage_arc.map(|storage_arc| PublishedData {
            agent: stores.agent,
            storage_arc,
            published_hashes,
        }))
    });
    futures::stream::iter(iter)
        .buffer_unordered(concurrency)
        .try_filter_map(|d| futures::future::ok(d))
        .try_collect()
        .await
}

async fn request_published_ops(env: &EnvRead) -> StateQueryResult<Vec<Arc<KitsuneOpHash>>> {
    Ok(env
        .async_reader(|txn| {
            // Collect all ops except StoreEntry's that are private.
            let r = txn
                .prepare(
                    "
                    SELECT
                    DhtOp.hash as dht_op_hash
                    FROM DhtOp
                    JOIN
                    Header ON DhtOp.header_hash = Header.hash
                    WHERE
                    DhtOp.is_authored = 1
                    AND
                    (DhtOp.type != :store_entry OR Header.private_entry = 0)
                ",
                )?
                .query_map(
                    named_params! {
                        ":store_entry": DhtOpType::StoreEntry,
                    },
                    |row| {
                        let h: DhtOpHash = row.get("dht_op_hash")?;
                        Ok(h.to_kitsune())
                    },
                )?
                .collect::<Result<_, _>>()?;
            StateQueryResult::Ok(r)
        })
        .await?)
}

async fn request_arc(env: &EnvRead, agent: &KitsuneAgent) -> StateQueryResult<Option<DhtArc>> {
    use holochain_sqlite::db::ReadManager;
    env.conn()?
        .with_reader(|txn| Ok(txn.p2p_get_agent(agent)?.map(|info| info.storage_arc)))
}
