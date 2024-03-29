//! Queries for the P2pAgentStore db
use futures::StreamExt;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_conductor_api::AgentInfoDump;
use holochain_conductor_api::P2pAgentsDump;
use holochain_p2p::dht::spacetime::Topology;
use holochain_p2p::dht::PeerStrat;
use holochain_p2p::dht::PeerView;
use holochain_p2p::dht_arc::DhtArc;
use holochain_p2p::kitsune_p2p::agent_store::AgentInfoSigned;
use holochain_p2p::AgentPubKeyExt;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_state::query::StateQueryError;
use std::sync::Arc;
use thiserror::Error;
use kitsune_p2p_types::bootstrap::AgentInfoPut;

use super::error::ConductorResult;

/// A set of agent information that are to be committed
/// with any other active batches.
pub struct P2pBatch {
    /// Agent information to be committed.
    pub peer_data: Vec<AgentInfoSigned>,
    /// The result of this commit.
    pub result_sender: tokio::sync::oneshot::Sender<Result<Vec<AgentInfoPut>, P2pBatchError>>,
}

#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum P2pBatchError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),
    #[error("Batch transaction failed {0}")]
    BatchFailed(String),
}

/// Inject multiple agent info entries into the peer store
pub async fn inject_agent_infos<'iter, I: IntoIterator<Item = &'iter AgentInfoSigned> + Send>(
    env: DbWrite<DbKindP2pAgents>,
    iter: I,
) -> StateMutationResult<()> {
    p2p_put_all(&env, iter.into_iter()).await?;
    Ok(())
}

/// Inject multiple agent info entries into the peer store in batches.
pub async fn p2p_put_all_batch(
    env: DbWrite<DbKindP2pAgents>,
    rx: tokio::sync::mpsc::Receiver<P2pBatch>,
) {
    let space = env.kind().0.clone();
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    let mut stream = stream.ready_chunks(100);
    while let Some(batch) = stream.next().await {
        let mut responses = Vec::with_capacity(batch.len());
        let (tx, rx) = tokio::sync::oneshot::channel();
        let result = env
            .write_async({
                let space = space.clone();
                move |txn| {
                    'batch: for P2pBatch {
                        peer_data: batch,
                        result_sender: response,
                    } in batch
                    {
                        let mut put_infos = Vec::with_capacity(batch.len());
                        for info in batch {
                            match p2p_put_single(space.clone(), txn, &info) {
                                Ok(put_info) => put_infos.push(put_info),
                                Err(e) => {
                                    responses.push((Err(e), response));
                                    continue 'batch;
                                }
                            }
                        }
                        responses.push((Ok(put_infos), response));
                    }
                    tx.send(responses).map_err(|_| {
                        DatabaseError::Other(anyhow::anyhow!(
                            "Failed to send response from background thread"
                        ))
                    })?;
                    DatabaseResult::Ok(())
                }
            })
            .await;
        let responses = rx.await;
        match result {
            Ok(_) => {
                if let Ok(responses) = responses {
                    for (result, response) in responses {
                        let _ = response.send(result.map_err(P2pBatchError::from));
                    }
                }
            }
            Err(e) => {
                if let Ok(responses) = responses {
                    for (_, response) in responses {
                        let _ = response.send(Err(P2pBatchError::BatchFailed(format!("{:?}", e))));
                    }
                }
            }
        }
    }
}

/// Helper function to get all the peer data from this conductor
pub async fn all_agent_infos(
    env: DbRead<DbKindP2pAgents>,
) -> StateQueryResult<Vec<AgentInfoSigned>> {
    Ok(env.p2p_list_agents().await?)
}

/// Helper function to get a single agent info
pub async fn get_single_agent_info(
    env: DbRead<DbKindP2pAgents>,
    _space: DnaHash,
    agent: AgentPubKey,
) -> StateQueryResult<Option<AgentInfoSigned>> {
    let agent = agent.to_kitsune();
    Ok(env.p2p_get_agent(&agent).await?)
}

/// Share all current agent infos known to all provided peer dbs with each other.
#[cfg(any(test, feature = "test_utils"))]
pub async fn exchange_peer_info(envs: Vec<DbWrite<DbKindP2pAgents>>) {
    use std::collections::HashSet;
    let mut all_infos: HashSet<AgentInfoSigned> = HashSet::new();

    for env in envs.iter() {
        let infos: HashSet<AgentInfoSigned> = all_agent_infos(env.clone().into())
            .await
            .unwrap()
            .into_iter()
            .collect();
        all_infos.extend(infos);
    }

    for env in envs.iter() {
        inject_agent_infos(env.clone(), all_infos.iter())
            .await
            .unwrap();
    }
}

/// Drop the specified agent keys from each conductor's peer db.
#[cfg(any(test, feature = "test_utils"))]
pub async fn forget_peer_info(
    all_envs: Vec<DbWrite<DbKindP2pAgents>>,
    agents_to_forget: impl IntoIterator<Item = &AgentPubKey>,
) {
    use kitsune_p2p_types::KAgent;

    let agents_to_forget: Vec<KAgent> = agents_to_forget
        .into_iter()
        .map(|a| a.to_kitsune())
        .collect();

    futures::future::join_all(all_envs.clone().into_iter().map(move |env| {
        let agents = agents_to_forget.clone();

        async move {
            for agent in agents.iter() {
                env.p2p_remove_agent(agent).await.unwrap();
            }
        }
    }))
    .await;
}

/// Interconnect provided pair of conductors via their peer store databases,
/// according to the connectivity matrix
#[cfg(any(test, feature = "test_utils"))]
pub async fn exchange_peer_info_sparse(
    envs: Vec<DbWrite<DbKindP2pAgents>>,
    connectivity: Vec<std::collections::HashSet<usize>>,
) {
    assert_eq!(envs.len(), connectivity.len());
    for (i, a) in envs.iter().enumerate() {
        let infos_a = all_agent_infos(a.clone().into()).await.unwrap();
        for (j, b) in envs.iter().enumerate() {
            if i == j {
                continue;
            }
            if !connectivity[j].contains(&i) {
                continue;
            }
            // let infos_b = all_agent_infos(b.clone().into()).await.unwrap();
            // inject_agent_infos(a.clone(), infos_b.iter()).await.unwrap();
            inject_agent_infos(b.clone(), infos_a.iter()).await.unwrap();
        }
    }
}

/// Reveal every agent in a single conductor to every agent in another.
#[cfg(any(test, feature = "test_utils"))]
pub async fn reveal_peer_info(
    observer_envs: Vec<DbWrite<DbKindP2pAgents>>,
    seen_envs: Vec<DbWrite<DbKindP2pAgents>>,
) {
    for observer in observer_envs.iter() {
        for seen in seen_envs.iter() {
            inject_agent_infos(
                observer.clone(),
                all_agent_infos(seen.clone().into()).await.unwrap().iter(),
            )
            .await
            .unwrap();
        }
    }
}

/// Get agent info for a single agent
pub async fn get_agent_info_signed(
    environ: DbRead<DbKindP2pAgents>,
    _kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    kitsune_agent: Arc<kitsune_p2p::KitsuneAgent>,
) -> ConductorResult<Option<AgentInfoSigned>> {
    Ok(environ.p2p_get_agent(&kitsune_agent).await?)
}

/// Get all agent info for a single space
pub async fn list_all_agent_info(
    environ: DbRead<DbKindP2pAgents>,
    _kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
) -> ConductorResult<Vec<AgentInfoSigned>> {
    Ok(environ.p2p_list_agents().await?)
}

/// Get all agent info for a single space near a basis loc
pub async fn list_all_agent_info_signed_near_basis(
    environ: DbRead<DbKindP2pAgents>,
    _kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    basis_loc: u32,
    limit: u32,
) -> ConductorResult<Vec<AgentInfoSigned>> {
    Ok(environ.p2p_query_near_basis(basis_loc, limit).await?)
}

/// Get the peer density an agent is currently seeing within
/// a given [`DhtArc`]
pub async fn query_peer_density(
    env: DbRead<DbKindP2pAgents>,
    topology: Topology,
    strat: PeerStrat,
    kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    dht_arc: DhtArc,
) -> ConductorResult<PeerView> {
    let now = now();
    let arcs = env.p2p_list_agents().await?;
    let arcs: Vec<_> = arcs
        .into_iter()
        .filter_map(|v| {
            if v.space == kitsune_space && !is_expired(now, &v) {
                Some(v.storage_arc)
            } else {
                None
            }
        })
        .collect();

    // contains is already checked in the iterator
    Ok(strat.view(topology, dht_arc, arcs.as_slice()))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn is_expired(now: u64, info: &AgentInfoSigned) -> bool {
    now >= info.expires_at_ms
}

/// Dump the agents currently in the peer store
pub async fn dump_state(
    env: DbRead<DbKindP2pAgents>,
    cell_id: Option<CellId>,
) -> StateQueryResult<P2pAgentsDump> {
    use std::fmt::Write;
    let cell_id = cell_id.map(|c| c.into_dna_and_agent()).map(|c| {
        (
            (c.0.clone(), holochain_p2p::space_holo_to_kit(c.0)),
            (c.1.clone(), holochain_p2p::agent_holo_to_kit(c.1)),
        )
    });
    let agent_infos = all_agent_infos(env).await?;
    let agent_infos = agent_infos.into_iter().filter(|a| match &cell_id {
        Some((s, _)) => s.1 == *a.space,
        None => true,
    });
    let mut this_agent_info = None;
    let mut peers = Vec::new();
    for info in agent_infos {
        let mut dump = String::new();

        use chrono::{DateTime, Duration, NaiveDateTime, Utc};
        let duration = Duration::try_milliseconds(info.signed_at_ms as i64).ok_or_else(|| {
            StateQueryError::Other("Agent info timestamp out of range".to_string())
        })?;
        let s = duration.num_seconds();
        let n = duration.clone().to_std().unwrap().subsec_nanos();
        // TODO FIXME
        #[allow(deprecated)]
        let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
        let duration = Duration::try_milliseconds(info.expires_at_ms as i64).ok_or_else(|| {
            StateQueryError::Other("Agent info timestamp out of range".to_string())
        })?;
        let s = duration.num_seconds();
        let n = duration.clone().to_std().unwrap().subsec_nanos();
        // TODO FIXME
        #[allow(deprecated)]
        let exp = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
        let now = Utc::now();

        writeln!(dump, "signed at {}", dt).ok();
        writeln!(
            dump,
            "expires at {} in {}mins",
            exp,
            (exp - now).num_minutes()
        )
        .ok();
        writeln!(dump, "urls: {:?}", info.url_list).ok();
        let info = AgentInfoDump {
            kitsune_agent: info.agent.clone(),
            kitsune_space: info.space.clone(),
            dump,
        };
        match &cell_id {
            Some((s, a)) if *info.kitsune_agent == a.1 && *info.kitsune_space == s.1 => {
                this_agent_info = Some(info);
            }
            None | Some(_) => peers.push(info),
        }
    }

    Ok(P2pAgentsDump {
        this_agent_info,
        this_dna: cell_id.clone().map(|(s, _)| s),
        this_agent: cell_id.clone().map(|(_, a)| a),
        peers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holochain_state::test_utils::test_p2p_agents_db;
    use kitsune_p2p_types::fixt::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_store_agent_info_signed() {
        holochain_trace::test_run().ok();

        let test_db = test_p2p_agents_db();
        let db = test_db.to_db();

        let agent_info_signed = fixt!(AgentInfoSigned, Predictable);

        p2p_put(&db, &agent_info_signed).await.unwrap();

        let ret = db.p2p_get_agent(&agent_info_signed.agent).await.unwrap();

        assert_eq!(ret, Some(agent_info_signed));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_agent_info_to_db() {
        holochain_trace::test_run().ok();
        let t_db = test_p2p_agents_db();
        let db = t_db.to_db();

        // - Check no data in the store to start
        let count = db.p2p_count_agents().await.unwrap();

        assert_eq!(count, 0);

        // - Get agents and space
        let agent_infos = AgentInfoSignedFixturator::new(Unpredictable)
            .take(5)
            .collect::<Vec<_>>();

        let mut expect = agent_infos.clone();
        expect.sort_by(|a, b| a.agent.partial_cmp(&b.agent).unwrap());

        // - Inject some data
        inject_agent_infos(db.clone(), agent_infos.iter())
            .await
            .unwrap();

        // - Check the same data is now in the store
        let mut agents = all_agent_infos(db.clone().into()).await.unwrap();

        agents.sort_by(|a, b| a.agent.partial_cmp(&b.agent).unwrap());

        assert_eq!(expect, agents);
    }
}
