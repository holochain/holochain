//! Queries for the P2pAgentStore db
use futures::StreamExt;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_conductor_api::AgentInfoDump;
use holochain_conductor_api::P2pAgentsDump;
use holochain_p2p::dht_arc::DhtArc;
use holochain_p2p::dht_arc::DhtArcBucket;
use holochain_p2p::dht_arc::PeerDensity;
use holochain_p2p::kitsune_p2p::agent_store::AgentInfoSigned;
use holochain_p2p::AgentPubKeyExt;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::StateMutationResult;
use holochain_state::prelude::StateQueryResult;
use holochain_types::prelude::*;
use holochain_zome_types::CellId;
use kitsune_p2p::KitsuneBinType;
use std::sync::Arc;
use thiserror::Error;

use super::error::ConductorResult;

/// A set of agent information that are to be committed
/// with any other active batches.
pub struct P2pBatch {
    /// Agent information to be committed.
    pub peer_data: Vec<AgentInfoSigned>,
    /// The result of this commit.
    pub result_sender: tokio::sync::oneshot::Sender<Result<(), P2pBatchError>>,
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
    env: EnvWrite,
    iter: I,
) -> StateMutationResult<()> {
    Ok(p2p_put_all(&env, iter.into_iter()).await?)
}

/// Inject multiple agent info entries into the peer store in batches.
pub async fn p2p_put_all_batch(env: EnvWrite, rx: tokio::sync::mpsc::Receiver<P2pBatch>) {
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    let mut stream = stream.ready_chunks(100);
    while let Some(batch) = stream.next().await {
        let mut responses = Vec::with_capacity(batch.len());
        let (tx, rx) = tokio::sync::oneshot::channel();
        let result = env
            .async_commit(move |mut txn| {
                'batch: for P2pBatch {
                    peer_data: batch,
                    result_sender: response,
                } in batch
                {
                    for info in batch {
                        match p2p_put_single(&mut txn, &info) {
                            Ok(_) => (),
                            Err(e) => {
                                responses.push((Err(e), response));
                                continue 'batch;
                            }
                        }
                    }
                    responses.push((Ok(()), response));
                }
                tx.send(responses)
                    .expect("We never drop the receiver before this send");
                DatabaseResult::Ok(())
            })
            .await;
        let responses = rx
            .await
            .expect("We never drop the sender before sending the responses");
        match result {
            Ok(_) => {
                for (result, response) in responses {
                    let _ = response.send(result.map_err(P2pBatchError::from));
                }
            }
            Err(e) => {
                for (_, response) in responses {
                    let _ = response.send(Err(P2pBatchError::BatchFailed(format!("{:?}", e))));
                }
            }
        }
    }
}

/// Helper function to get all the peer data from this conductor
pub async fn all_agent_infos(env: EnvRead) -> StateQueryResult<Vec<AgentInfoSigned>> {
    env.async_reader(|r| Ok(r.p2p_list_agents()?)).await
}

/// Helper function to get a single agent info
pub async fn get_single_agent_info(
    env: EnvRead,
    _space: DnaHash,
    agent: AgentPubKey,
) -> StateQueryResult<Option<AgentInfoSigned>> {
    let agent = agent.to_kitsune();
    env.async_reader(move |r| Ok(r.p2p_get_agent(&agent)?))
        .await
}

/// Interconnect every provided pair of conductors via their peer store databases
#[cfg(any(test, feature = "test_utils"))]
pub async fn exchange_peer_info(envs: Vec<EnvWrite>) {
    for (i, a) in envs.iter().enumerate() {
        for (j, b) in envs.iter().enumerate() {
            if i == j {
                continue;
            }
            inject_agent_infos(
                a.clone(),
                all_agent_infos(b.clone().into()).await.unwrap().iter(),
            )
            .await
            .unwrap();
            inject_agent_infos(
                b.clone(),
                all_agent_infos(a.clone().into()).await.unwrap().iter(),
            )
            .await
            .unwrap();
        }
    }
}

/// Get agent info for a single agent
pub fn get_agent_info_signed(
    environ: EnvWrite,
    _kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    kitsune_agent: Arc<kitsune_p2p::KitsuneAgent>,
) -> ConductorResult<Option<AgentInfoSigned>> {
    Ok(environ.conn()?.p2p_get_agent(&kitsune_agent)?)
}

/// Get all agent info for a single space
pub fn list_all_agent_info(
    environ: EnvWrite,
    _kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
) -> ConductorResult<Vec<AgentInfoSigned>> {
    Ok(environ.conn()?.p2p_list_agents()?)
}

/// Get all agent info for a single space near a basis loc
pub fn list_all_agent_info_signed_near_basis(
    environ: EnvWrite,
    _kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    basis_loc: u32,
    limit: u32,
) -> ConductorResult<Vec<AgentInfoSigned>> {
    Ok(environ.conn()?.p2p_query_near_basis(basis_loc, limit)?)
}

/// Get the peer density an agent is currently seeing within
/// a given [`DhtArc`]
pub fn query_peer_density(
    env: EnvWrite,
    kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    dht_arc: DhtArc,
) -> ConductorResult<PeerDensity> {
    let now = now();
    let arcs = env.conn()?.p2p_list_agents()?;
    let arcs = arcs
        .into_iter()
        .filter_map(|v| {
            if dht_arc.contains(v.agent.get_loc()) {
                if v.space == kitsune_space && !is_expired(now, &v) {
                    Some(v.storage_arc)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // contains is already checked in the iterator
    let bucket = DhtArcBucket::new_unchecked(dht_arc, arcs);

    Ok(bucket.density())
}

/// Put single agent info into store
pub async fn put_agent_info_signed(
    environ: EnvWrite,
    agent_info_signed: kitsune_p2p::agent_store::AgentInfoSigned,
) -> ConductorResult<()> {
    Ok(p2p_put(&environ, &agent_info_signed).await?)
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
pub async fn dump_state(env: EnvRead, cell_id: Option<CellId>) -> StateQueryResult<P2pAgentsDump> {
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
        let duration = Duration::milliseconds(info.signed_at_ms as i64);
        let s = duration.num_seconds() as i64;
        let n = duration.clone().to_std().unwrap().subsec_nanos();
        let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
        let duration = Duration::milliseconds(info.expires_at_ms as i64);
        let s = duration.num_seconds() as i64;
        let n = duration.clone().to_std().unwrap().subsec_nanos();
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
    use holochain_state::test_utils::test_p2p_agent_store_env;
    use kitsune_p2p::fixt::AgentInfoSignedFixturator;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_store_agent_info_signed() {
        observability::test_run().ok();

        let test_env = test_p2p_agent_store_env();
        let env = test_env.env();

        let agent_info_signed = fixt!(AgentInfoSigned, Predictable);

        p2p_put(&env, &agent_info_signed).await.unwrap();

        let ret = env
            .conn()
            .unwrap()
            .p2p_get_agent(&agent_info_signed.agent)
            .unwrap();

        assert_eq!(ret, Some(agent_info_signed));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_agent_info_to_peer_env() {
        observability::test_run().ok();
        let t_env = test_p2p_agent_store_env();
        let env = t_env.env();

        // - Check no data in the store to start
        let count = env.conn().unwrap().p2p_list_agents().unwrap().len();

        assert_eq!(count, 0);

        // - Get agents and space
        let agent_infos = AgentInfoSignedFixturator::new(Unpredictable)
            .take(5)
            .collect::<Vec<_>>();

        let mut expect = agent_infos.clone();
        expect.sort_by(|a, b| a.agent.partial_cmp(&b.agent).unwrap());

        // - Inject some data
        inject_agent_infos(env.clone(), agent_infos.iter())
            .await
            .unwrap();

        // - Check the same data is now in the store
        let mut agents = all_agent_infos(env.clone().into()).await.unwrap();

        agents.sort_by(|a, b| a.agent.partial_cmp(&b.agent).unwrap());

        assert_eq!(expect, agents);
    }
}
