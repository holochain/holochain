//! Utilities for testing the consistency of the dht.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use futures::stream::StreamExt;
use holo_hash::{DhtOpHash, DnaHash};
use holochain_p2p::{
    dht_arc::{DhtArc, DhtLocation},
    AgentPubKeyExt, DhtOpHashExt, DnaHashExt,
};
use holochain_sqlite::{
    db::{AsP2pStateTxExt, DbKindAuthored, DbKindDht, DbKindP2pAgentStore},
    prelude::DatabaseResult,
};
use holochain_state::prelude::StateQueryResult;
use holochain_types::{dht_op::DhtOpType, env::DbRead};
use kitsune_p2p::{KitsuneAgent, KitsuneOpHash};
use kitsune_p2p_types::consistency::*;
use rusqlite::named_params;

use crate::conductor::ConductorHandle;

struct Stores {
    agent: Arc<KitsuneAgent>,
    authored_env: DbRead<DbKindAuthored>,
    p2p_env: DbRead<DbKindP2pAgentStore>,
}

#[derive(Clone)]
struct Reporter(tokio::sync::mpsc::Sender<SessionMessage>, Arc<KitsuneAgent>);

const CONCURRENCY: usize = 100;

/// A helper for checking consistency of all published ops for all cells in all conductors
/// has reached consistency in a sharded context.
pub async fn local_machine_session(conductors: &[ConductorHandle], timeout: Duration) {
    // For each space get all the cells, their env and the p2p envs.
    let mut spaces = HashMap::new();
    for (i, c) in conductors.iter().enumerate() {
        for cell_id in c.list_cell_ids(None) {
            let space = spaces
                .entry(cell_id.dna_hash().clone())
                .or_insert_with(|| vec![None; conductors.len()]);
            if space[i].is_none() {
                let p2p_env: DbRead<DbKindP2pAgentStore> =
                    c.get_p2p_env(cell_id.dna_hash().to_kitsune()).into();
                space[i] = Some((p2p_env, Vec::new()));
            }
            space[i].as_mut().unwrap().1.push((
                c.get_authored_env(cell_id.dna_hash()).unwrap().into(),
                c.get_dht_env(cell_id.dna_hash()).unwrap().into(),
                cell_id.agent_pubkey().to_kitsune(),
            ));
        }
    }

    // Run a consistency session for each space.
    for (_, conductors) in spaces {
        // The agents we need to wait for.
        let mut wait_for_agents = HashSet::new();

        // Maps to environments.
        let mut agent_env_map = HashMap::new();
        let mut agent_p2p_map = HashMap::new();

        // All the agents that should be held.
        let mut all_agents = Vec::new();
        // All the op hashes that should be held.
        let mut all_hashes = Vec::new();
        let (tx, rx) = tokio::sync::mpsc::channel(1000);

        // Gather the expected agents and op hashes from each conductor.
        for (p2p_env, agents) in conductors.into_iter().flatten() {
            wait_for_agents.extend(agents.iter().map(|(_, _, agent)| agent.clone()));
            agent_env_map.extend(agents.iter().cloned().map(|(_, dht, agent)| (agent, dht)));
            agent_p2p_map.extend(agents.iter().cloned().map(|(_, _, a)| (a, p2p_env.clone())));
            let (a, h) = gather_conductor_data(p2p_env, agents).await;
            all_agents.extend(a);
            all_hashes.extend(h);
        }

        // Spawn a background task that will run each
        // cells self consistency check against the data that
        // they are expected to hold.
        tokio::spawn(expect_all(
            tx,
            timeout,
            all_agents,
            all_hashes,
            agent_env_map,
            agent_p2p_map,
        ));

        // Wait up to the timeout for all the agents to report success.
        wait_for_consistency(rx, wait_for_agents, timeout).await;
    }
}

/// Get consistency for a particular hash.
pub async fn local_machine_session_with_hashes(
    handles: Vec<&ConductorHandle>,
    hashes: impl Iterator<Item = (DhtLocation, DhtOpHash)>,
    space: &DnaHash,
    timeout: Duration,
) {
    // Grab the environments and cells for each conductor in this space.
    let mut conductors = vec![None; handles.len()];
    for (i, c) in handles.iter().enumerate() {
        for cell_id in c.list_cell_ids(None) {
            if cell_id.dna_hash() != space {
                continue;
            }
            if conductors[i].is_none() {
                let p2p_env: DbRead<DbKindP2pAgentStore> =
                    c.get_p2p_env(cell_id.dna_hash().to_kitsune()).into();
                conductors[i] = Some((p2p_env, Vec::new()));
            }
            conductors[i].as_mut().unwrap().1.push((
                c.get_dht_env(cell_id.dna_hash()).unwrap().into(),
                cell_id.agent_pubkey().to_kitsune(),
            ));
        }
    }

    // Convert the hashes to kitsune.
    let all_hashes = hashes
        .into_iter()
        .map(|(l, h)| (l, h.into_kitsune_raw()))
        .collect::<Vec<_>>();
    // The agents we need to wait for.
    let mut wait_for_agents = HashSet::new();

    // Maps to environments.
    let mut agent_env_map = HashMap::new();
    let mut agent_p2p_map = HashMap::new();

    // All the agents that should be held.
    let mut all_agents = Vec::new();
    let (tx, rx) = tokio::sync::mpsc::channel(1000);

    // Gather the expected agents from each conductor.
    for (p2p_env, agents) in conductors.into_iter().flatten() {
        wait_for_agents.extend(agents.iter().map(|(_, agent)| agent.clone()));
        agent_env_map.extend(agents.iter().cloned().map(|(e, a)| (a, e)));
        agent_p2p_map.extend(agents.iter().cloned().map(|(_, a)| (a, p2p_env.clone())));
        for (_, agent) in &agents {
            if let Some(storage_arc) = request_arc(&p2p_env, (**agent).clone()).await.unwrap() {
                all_agents.push((agent.clone(), storage_arc));
            }
        }
    }

    // Spawn a background task that will run each
    // cells self consistency check against the data that
    // they are expected to hold.
    tokio::spawn(expect_all(
        tx,
        timeout,
        all_agents,
        all_hashes,
        agent_env_map,
        agent_p2p_map,
    ));

    // Wait up to the timeout for all the agents to report success.
    wait_for_consistency(rx, wait_for_agents, timeout).await;
}

impl Reporter {
    /// Send a report back.
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

/// Wait for all agents to report success, timeout or failure.
/// Additionally print out debug tracing with some statistics.
#[tracing::instrument(skip(rx, agents))]
async fn wait_for_consistency(
    mut rx: tokio::sync::mpsc::Receiver<SessionMessage>,
    mut agents: HashSet<Arc<KitsuneAgent>>,
    timeout: Duration,
) {
    // When the session began.
    let start = tokio::time::Instant::now();
    // When the session is expected to end with a buffer to allow agents to timeout first.
    let deadline = tokio::time::Instant::now() + timeout + Duration::from_secs(1);

    // Stats.
    let total_agents = agents.len();
    let mut timeouts = 0;
    let mut errors = 0;
    let mut success = 0;
    let mut average_time = Duration::default();
    let mut amount_held = HashMap::new();
    let avg_held = |amount_held: &HashMap<_, _>| {
        let (p_agent_held, p_hash_held) = amount_held.values().fold(
            (0.0, 0.0),
            |(mut p_agent_held, mut p_hash_held), (ma, ea, mh, eh)| {
                p_agent_held += (*ea - *ma) as f32 / *ea as f32;
                p_hash_held += (*eh - *mh) as f32 / *eh as f32;
                (p_agent_held, p_hash_held)
            },
        );
        let avg_agent_held = p_agent_held / amount_held.len() as f32 * 100.0;
        let avg_hash_held = p_hash_held / amount_held.len() as f32 * 100.0;
        (avg_agent_held.round(), avg_hash_held.round())
    };

    // While we haven't timed out collect messages from all agents, print traces and update stats.
    while let Ok(Some(SessionMessage { from, report })) =
        tokio::time::timeout_at(deadline, rx.recv()).await
    {
        // Incase the future is always ready we need to check for timeout here as well.
        if tokio::time::Instant::now() > deadline {
            break;
        }
        match report {
            SessionReport::KeepAlive {
                missing_agents,
                missing_hashes,
                expected_agents,
                expected_hashes,
            } => {
                let e = amount_held
                    .entry(from.clone())
                    .or_insert_with(|| (0, 0, 0, 0));
                e.0 = missing_agents;
                e.1 = expected_agents;
                e.2 = missing_hashes;
                e.3 = expected_hashes;
                tracing::debug!(
                    "{:?} is still missing {} of {} agents and {} of {} hashes",
                    from,
                    missing_agents,
                    expected_agents,
                    missing_hashes,
                    expected_hashes,
                );
            }
            SessionReport::Complete { elapsed_ms } => {
                agents.remove(&from);
                tracing::debug!("{:?} has reached consistency in {}ms", from, elapsed_ms);
                average_time += Duration::from_millis(elapsed_ms as u64);
                success += 1;
                if agents.is_empty() {
                    break;
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
                timeouts += 1;
                if agents.is_empty() {
                    break;
                }
            }
            SessionReport::Error { error } => {
                agents.remove(&from);
                tracing::debug!(
                    "{:?} has failed the consistency session with error {}",
                    from,
                    error
                );
                errors += 1;
                if agents.is_empty() {
                    break;
                }
            }
        }
        let (avg_agent_held, avg_hash_held) = avg_held(&amount_held);
        tracing::debug!(
            "{} of {} agents have still not reached consistency in {:?}. The average consistency is currently reached in {:?}. {}% agents held, {}% hashes held.",
            agents.len(),
            total_agents,
            start.elapsed(),
            average_time.checked_div(success as u32).unwrap_or_default(),
            avg_agent_held,
            avg_hash_held,
        );
    }
    if tokio::time::Instant::now() > deadline {
        timeouts += agents.len();
        agents.clear();
        tracing::debug!(
            "Timed out with {} of {} agents have still not reaching consistency",
            agents.len(),
            total_agents
        );
    }
    let (avg_agent_held, avg_hash_held) = avg_held(&amount_held);
    tracing::debug!(
        "
REPORT:
Total elapsed: {:?}
Successful agents: {} in an average of {:?}
Timed out agents: {}
Failed out agents: {}
Total agents: {}
Average agents held: {}%.
Average hashes held: {}%.
        ",
        start.elapsed(),
        success,
        average_time.checked_div(success as u32).unwrap_or_default(),
        timeouts,
        errors,
        total_agents,
        avg_agent_held,
        avg_hash_held,
    );
}

/// Gather all the published op hashes and agents from a conductor.
async fn gather_conductor_data(
    p2p_env: DbRead<DbKindP2pAgentStore>,
    agents: Vec<(DbRead<DbKindAuthored>, DbRead<DbKindDht>, Arc<KitsuneAgent>)>,
) -> (
    Vec<(Arc<KitsuneAgent>, DhtArc)>,
    Vec<(DhtLocation, KitsuneOpHash)>,
) {
    // Create the stores iterator with the environments to search.
    let stores = agents
        .iter()
        .cloned()
        .map(|(authored_env, _, agent)| Stores {
            agent,
            authored_env,
            p2p_env: p2p_env.clone(),
        });
    let all_published_data = gather_published_data(stores, CONCURRENCY)
        .await
        .expect("Failed to gather published data from conductor");
    let mut all_hashes = Vec::new();
    let mut all_agents = Vec::with_capacity(all_published_data.len());

    // Collect all the published hashes and agents.
    for PublishedData {
        agent,
        storage_arc,
        published_hashes,
    } in all_published_data
    {
        all_hashes.extend(published_hashes);
        all_agents.push((agent, storage_arc));
    }
    (all_agents, all_hashes)
}

/// Generate the consistency session and then check all agents concurrently.
async fn expect_all(
    tx: tokio::sync::mpsc::Sender<SessionMessage>,
    timeout: Duration,
    all_agents: Vec<(Arc<KitsuneAgent>, DhtArc)>,
    all_hashes: Vec<(DhtLocation, KitsuneOpHash)>,
    agent_env_map: HashMap<Arc<KitsuneAgent>, DbRead<DbKindDht>>,
    agent_p2p_map: HashMap<Arc<KitsuneAgent>, DbRead<DbKindP2pAgentStore>>,
) {
    let iter = generate_session(&all_agents, &all_hashes, timeout, agent_env_map);
    check_all(iter, tx, agent_p2p_map).await;
}

/// Generate the consistency sessions for each agent along with their environments.
/// This is where we check which agents should be holding which hashes and agents.
fn generate_session<'iter>(
    all_agents: &'iter Vec<(Arc<KitsuneAgent>, DhtArc)>,
    all_hashes: &'iter Vec<(DhtLocation, KitsuneOpHash)>,
    timeout: Duration,
    agent_env_map: HashMap<Arc<KitsuneAgent>, DbRead<DbKindDht>>,
) -> impl Iterator<Item = (Arc<KitsuneAgent>, ConsistencySession, DbRead<DbKindDht>)> + 'iter {
    all_agents
        .iter()
        .map(move |(agent, arc)| {
            let mut published_hashes = Vec::new();
            let mut expected_agents = Vec::new();
            for (basis_loc, hash) in all_hashes.iter() {
                if arc.contains(*basis_loc) {
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

/// Concurrently check all agents for consistency.
/// Report back the results on the channel.
/// Checks will report timeouts and failures.
async fn check_all(
    iter: impl Iterator<Item = (Arc<KitsuneAgent>, ConsistencySession, DbRead<DbKindDht>)>,
    tx: tokio::sync::mpsc::Sender<SessionMessage>,
    agent_p2p_map: HashMap<Arc<KitsuneAgent>, DbRead<DbKindP2pAgentStore>>,
) {
    futures::stream::iter(iter)
        .for_each_concurrent(CONCURRENCY, |(agent, expected_session, dht_env)| {
            let tx = tx.clone();
            let p2p_env = agent_p2p_map
                .get(&agent)
                .cloned()
                .expect("Must contain all p2p envs, this is a bug.");
            let reporter = Reporter(tx, agent);
            check_expected_data(reporter, expected_session, dht_env, p2p_env)
        })
        .await;
}

/// Check the expected data against for a single agent.
async fn check_expected_data(
    reporter: Reporter,
    session: ConsistencySession,
    dht_env: DbRead<DbKindDht>,
    p2p_env: DbRead<DbKindP2pAgentStore>,
) {
    if let Err(e) = check_expected_data_inner(reporter.clone(), session, dht_env, p2p_env).await {
        reporter
            .send_report(SessionReport::Error {
                error: e.to_string(),
            })
            .await;
    }
}

/// The check expected data inner loop.
/// This runs for each agent until success, failure or timeout.
/// All outcomes are reported back on the channel.
async fn check_expected_data_inner(
    reporter: Reporter,
    session: ConsistencySession,
    dht_env: DbRead<DbKindDht>,
    p2p_env: DbRead<DbKindP2pAgentStore>,
) -> DatabaseResult<()> {
    // Unpack the session.
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

    // When we started.
    let start = tokio::time::Instant::now();
    // When we should finish.
    let deadline = tokio::time::Instant::now() + timeout;
    // The last time we sent a keep alive.
    let mut last_keep_alive = start;
    // How frequently we should poll the database.
    let mut frequency = tokio::time::interval(frequency);
    frequency.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    // The agents and hashes we are still missing.
    let mut missing_agents = Vec::with_capacity(expected_agents.len());
    let mut missing_hashes = Vec::with_capacity(expected_hashes.len());

    // If we have not timed out then check at the set frequency.
    while tokio::time::timeout_at(deadline, frequency.tick())
        .await
        .is_ok()
    {
        // If the frequency interval is always ready the timeout
        // won't check so we also need to check for timeout here.
        if tokio::time::Instant::now() > deadline {
            break;
        }

        // Check the agents.
        missing_agents = check_agents(&p2p_env, &expected_agents).await?.collect();

        // Check the hashes.
        check_hashes(&dht_env, &mut expected_hashes, &mut missing_hashes).await?;

        // If both are now empty we report success.
        if missing_agents.is_empty() && missing_hashes.is_empty() {
            reporter
                .send_report(SessionReport::Complete {
                    elapsed_ms: start.elapsed().as_millis() as u32,
                })
                .await;
            return Ok(());
        }

        // If it's time to send a keep alive then do so now.
        if keep_alive.map_or(false, |k| last_keep_alive.elapsed() > k) {
            reporter
                .send_report(SessionReport::KeepAlive {
                    missing_agents: missing_agents.len() as u32,
                    expected_agents: expected_agents.len() as u32,
                    missing_hashes: missing_hashes.len() as u32,
                    expected_hashes: expected_hashes.len() as u32,
                })
                .await;

            // Update the last keep alive time.
            last_keep_alive = tokio::time::Instant::now();
        }
    }

    // We have not succeeded by now so we have timed out.
    reporter
        .send_report(SessionReport::Timeout {
            missing_agents: missing_agents.into_iter().cloned().collect(),
            missing_hashes,
        })
        .await;
    Ok(())
}

/// Check the agent is holding the expected agents in their peer store.
// Seems these lifetimes are actually needed.
#[allow(clippy::needless_lifetimes)]
async fn check_agents<'iter>(
    p2p_env: &DbRead<DbKindP2pAgentStore>,
    expected_agents: &'iter [Arc<KitsuneAgent>],
) -> DatabaseResult<impl Iterator<Item = &'iter Arc<KitsuneAgent>> + 'iter> {
    // Poll the peer database for the currently held agents.
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

    // Filter out the currently held agents from the expected agents to return any missing.
    Ok(expected_agents
        .iter()
        .filter(move |a| !agents_held.contains(&(**a))))
}

/// Check the op hashes we are meant to be holding.
async fn check_hashes(
    dht_env: &DbRead<DbKindDht>,
    expected_hashes: &mut Vec<KitsuneOpHash>,
    missing_hashes: &mut Vec<KitsuneOpHash>,
) -> DatabaseResult<()> {
    // Clear the missing hashes from the last check. This doesn't affect allocation.
    missing_hashes.clear();

    // We need to swap these hashes so we can move them into the async_reader
    // without reallocating.
    let expected = std::mem::replace(expected_hashes, Vec::with_capacity(0));
    let mut missing = std::mem::replace(missing_hashes, Vec::with_capacity(0));

    // Poll the vault database for each expected hashes existence.
    let mut r = dht_env
                .async_reader(move |txn| {
                    for hash in &expected {
                        // TODO: This might be too slow, could instead save the holochain hash versions.
                        let h_hash: DhtOpHash = DhtOpHashExt::from_kitsune_raw(hash.clone());
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

/// Concurrently Gather all published op hashes and agent's storage arcs.
async fn gather_published_data(
    iter: impl Iterator<Item = Stores>,
    concurrency: usize,
) -> StateQueryResult<Vec<PublishedData>> {
    use futures::stream::TryStreamExt;
    let iter = iter.map(|stores| async move {
        let published_hashes = request_published_ops(&stores.authored_env).await?;
        let storage_arc = request_arc(&stores.p2p_env, (*stores.agent).clone()).await?;
        Ok(storage_arc.map(|storage_arc| PublishedData {
            agent: stores.agent,
            storage_arc,
            published_hashes,
        }))
    });
    futures::stream::iter(iter)
        .buffer_unordered(concurrency)
        .try_filter_map(futures::future::ok)
        .try_collect()
        .await
}

/// Request the published hashes for the given agent.
async fn request_published_ops(
    env: &DbRead<DbKindAuthored>,
) -> StateQueryResult<Vec<(DhtLocation, KitsuneOpHash)>> {
    Ok(env
        .async_reader(|txn| {
            // Collect all ops except StoreEntry's that are private.
            let r = txn
                .prepare(
                    "
                    SELECT
                    DhtOp.hash as dht_op_hash,
                    DhtOp.storage_center_loc as loc
                    FROM DhtOp
                    JOIN
                    Header ON DhtOp.header_hash = Header.hash
                    WHERE
                    (DhtOp.type != :store_entry OR Header.private_entry = 0)
                ",
                )?
                .query_map(
                    named_params! {
                        ":store_entry": DhtOpType::StoreEntry,
                    },
                    |row| {
                        let h: DhtOpHash = row.get("dht_op_hash")?;
                        let loc: u32 = row.get("loc")?;
                        Ok((loc.into(), h.into_kitsune_raw()))
                    },
                )?
                .collect::<Result<_, _>>()?;
            StateQueryResult::Ok(r)
        })
        .await?)
}

/// Request the storage arc for the given agent.
async fn request_arc(
    env: &DbRead<DbKindP2pAgentStore>,
    agent: KitsuneAgent,
) -> StateQueryResult<Option<DhtArc>> {
    env.async_reader(move |txn| Ok(txn.p2p_get_agent(&agent)?.map(|info| info.storage_arc)))
        .await
}
