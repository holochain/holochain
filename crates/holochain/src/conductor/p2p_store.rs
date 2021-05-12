//! A simple KvBuf for AgentInfoSigned.

use fallible_iterator::FallibleIterator;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_conductor_api::AgentInfoDump;
use holochain_conductor_api::P2pStateDump;
use holochain_p2p::dht_arc::DhtArc;
use holochain_p2p::dht_arc::DhtArcBucket;
use holochain_p2p::dht_arc::PeerDensity;
use holochain_p2p::kitsune_p2p::agent_store::AgentInfoSigned;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::StateMutationResult;
use holochain_state::prelude::StateQueryResult;
use holochain_types::prelude::*;
use holochain_zome_types::CellId;
use kitsune_p2p::KitsuneBinType;
use std::convert::TryFrom;
use std::sync::Arc;

use super::error::ConductorResult;

/// Inject multiple agent info entries into the peer store
pub fn inject_agent_infos<I: IntoIterator<Item = AgentInfoSigned> + Send>(
    env: EnvWrite,
    iter: I,
) -> StateMutationResult<()> {
    Ok(env.conn()?.with_commit(|writer| {
        for agent_info_signed in iter {
            holochain_state::agent_info::put(writer, agent_info_signed)?;
        }
        StateMutationResult::Ok(())
    })?)
}

/// Helper function to get all the peer data from this conductor
pub fn all_agent_infos(env: EnvRead) -> StateQueryResult<Vec<AgentInfoSigned>> {
    fresh_reader!(env, |r| { holochain_state::agent_info::get_all_values(&r) })
}

/// Helper function to get a single agent info
pub fn get_single_agent_info(
    env: EnvRead,
    space: DnaHash,
    agent: AgentPubKey,
) -> StateQueryResult<Option<AgentInfoSigned>> {
    fresh_reader!(env, |r| {
        holochain_state::agent_info::get_agent_info(&r, space, agent)
    })
}

/// Interconnect every provided pair of conductors via their peer store databases
#[cfg(any(test, feature = "test_utils"))]
pub fn exchange_peer_info(envs: Vec<EnvWrite>) {
    for (i, a) in envs.iter().enumerate() {
        for (j, b) in envs.iter().enumerate() {
            if i == j {
                continue;
            }
            inject_agent_infos(a.clone(), all_agent_infos(b.clone().into()).unwrap()).unwrap();
            inject_agent_infos(b.clone(), all_agent_infos(a.clone().into()).unwrap()).unwrap();
        }
    }
}

/// Get agent info for a single agent
pub fn get_agent_info_signed(
    environ: EnvWrite,
    kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    kitsune_agent: Arc<kitsune_p2p::KitsuneAgent>,
) -> ConductorResult<Option<AgentInfoSigned>> {
    environ.conn()?.with_commit(|writer| {
        let res =
            holochain_state::agent_info::get(writer, (&*kitsune_space, &*kitsune_agent).into())?;

        let res = match res {
            None => return Ok(None),
            Some(res) => res,
        };

        let info = kitsune_p2p::agent_store::AgentInfo::try_from(&res)?;
        let now = now();

        if is_expired(now, &info) {
            holochain_state::agent_info::delete(writer, (&*kitsune_space, &*kitsune_agent).into())?;
            return Ok(None);
        }

        Ok(Some(res))
    })
}

/// Get agent info for a single space
pub fn query_agent_info_signed(
    environ: EnvWrite,
    kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
) -> ConductorResult<Vec<AgentInfoSigned>> {
    let mut out = Vec::new();
    environ.conn()?.with_commit(|writer| {
        let mut expired = Vec::new();

        {
            let iter = holochain_state::agent_info::get_all(writer)?.into_iter();

            let now = now();

            for (k, v) in iter {
                let info = kitsune_p2p::agent_store::AgentInfo::try_from(&v)?;
                if is_expired(now, &info) {
                    expired.push(k);
                } else if info.as_space_ref() == kitsune_space.as_ref() {
                    out.push(v);
                }
            }
        }

        if !expired.is_empty() {
            for exp in expired {
                holochain_state::agent_info::delete(writer, exp)?;
            }
        }

        ConductorResult::Ok(())
    })?;

    Ok(out)
}

/// Get the peer density an agent is currently seeing within
/// a given [`DhtArc`]
pub fn query_peer_density(
    env: EnvWrite,
    kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    dht_arc: DhtArc,
) -> ConductorResult<PeerDensity> {
    let now = now();
    let arcs = fresh_reader!(env, |r| {
        fallible_iterator::convert(
            holochain_state::agent_info::get_all_values(&r)?
                .into_iter()
                .map(ConductorResult::Ok),
        )
        .filter_map(|v| {
            if dht_arc.contains(v.as_agent_ref().get_loc()) {
                let info = kitsune_p2p::agent_store::AgentInfo::try_from(&v)?;
                if info.as_space_ref() == kitsune_space.as_ref() && !is_expired(now, &info) {
                    Ok(Some(info.dht_arc()?))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        })
        .collect()
    })?;

    // contains is already checked in the iterator
    let bucket = DhtArcBucket::new_unchecked(dht_arc, arcs);

    Ok(bucket.density())
}

/// Put single agent info into store
pub fn put_agent_info_signed(
    environ: EnvWrite,
    agent_info_signed: kitsune_p2p::agent_store::AgentInfoSigned,
) -> ConductorResult<()> {
    Ok(environ
        .conn()?
        .with_commit(|writer| holochain_state::agent_info::put(writer, agent_info_signed))?)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn is_expired(now: u64, info: &kitsune_p2p::agent_store::AgentInfo) -> bool {
    info.signed_at_ms()
        .checked_add(info.expires_after_ms())
        .map(|expires| expires <= now)
        .unwrap_or(true)
}

/// Dump the agents currently in the peer store
pub fn dump_state(env: EnvRead, cell_id: Option<CellId>) -> StateQueryResult<P2pStateDump> {
    use std::fmt::Write;
    let cell_id = cell_id.map(|c| c.into_dna_and_agent()).map(|c| {
        (
            (c.0.clone(), holochain_p2p::space_holo_to_kit(c.0)),
            (c.1.clone(), holochain_p2p::agent_holo_to_kit(c.1)),
        )
    });
    let agent_infos = all_agent_infos(env)?;
    let agent_infos =
        agent_infos.into_iter().filter_map(
            |a| match kitsune_p2p::agent_store::AgentInfo::try_from(&a) {
                Ok(a) => match &cell_id {
                    Some((s, _)) => {
                        if s.1 == *a.as_space_ref() {
                            Some(a)
                        } else {
                            None
                        }
                    }
                    None => Some(a),
                },
                Err(e) => {
                    tracing::error!(failed_to_deserialize_agent_info = ?e);
                    None
                }
            },
        );
    let mut this_agent_info = None;
    let mut peers = Vec::new();
    for info in agent_infos {
        let mut dump = String::new();

        use chrono::{DateTime, Duration, NaiveDateTime, Utc};
        let duration = Duration::milliseconds(info.signed_at_ms() as i64);
        let s = duration.num_seconds() as i64;
        let n = duration.clone().to_std().unwrap().subsec_nanos();
        let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
        let exp = dt + Duration::milliseconds(info.expires_after_ms() as i64);
        let now = Utc::now();

        writeln!(dump, "signed at {}", dt).ok();
        writeln!(
            dump,
            "expires at {} in {}mins",
            exp,
            (exp - now).num_minutes()
        )
        .ok();
        writeln!(dump, "urls: {:?}", info.as_urls_ref()).ok();
        let info = AgentInfoDump {
            kitsune_agent: info.as_agent_ref().clone(),
            kitsune_space: info.as_space_ref().clone(),
            dump,
        };
        match &cell_id {
            Some((s, a)) if info.kitsune_agent == a.1 && info.kitsune_space == s.1 => {
                this_agent_info = Some(info);
            }
            None | Some(_) => peers.push(info),
        }
    }

    Ok(P2pStateDump {
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
    use holochain_sqlite::db::ReadManager;
    use holochain_sqlite::db::WriteManager;
    use holochain_state::agent_info::AgentKvKey;
    use holochain_state::test_utils::test_p2p_env;
    use kitsune_p2p::fixt::AgentInfoFixturator;
    use kitsune_p2p::fixt::AgentInfoSignedFixturator;
    use kitsune_p2p::KitsuneBinType;
    use std::convert::TryInto;

    #[test]
    fn kv_key_from() {
        let agent_info = fixt!(AgentInfo);

        let kv_key = AgentKvKey::from(&agent_info);

        let bytes = kv_key.as_ref().to_owned();

        assert_eq!(&bytes[..32], agent_info.as_space_ref().get_bytes(),);

        assert_eq!(&bytes[32..], agent_info.as_agent_ref().get_bytes(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_store_agent_info_signed() {
        observability::test_run().ok();

        let test_env = test_p2p_env();
        let env = test_env.env();

        let agent_info_signed = fixt!(AgentInfoSigned);

        env.conn()
            .unwrap()
            .with_commit(|txn| holochain_state::agent_info::put(txn, agent_info_signed.clone()))
            .unwrap();

        env.conn().unwrap().with_reader_test(|txn| {
            let ret =
                holochain_state::agent_info::get(&txn, (&agent_info_signed).try_into().unwrap())
                    .unwrap();

            assert_eq!(ret, Some(agent_info_signed),);
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_agent_info_to_peer_env() {
        observability::test_run().ok();
        let t_env = test_p2p_env();
        let env = t_env.env();

        // - Check no data in the store to start
        let count = env
            .conn()
            .unwrap()
            .with_reader_test(|txn| holochain_state::agent_info::get_all(&txn).unwrap().len());

        assert_eq!(count, 0);

        // - Get agents and space
        let agent_infos = AgentInfoSignedFixturator::new(Unpredictable)
            .take(5)
            .collect::<Vec<_>>();

        let mut expect = agent_infos.clone();
        expect.sort();

        // - Inject some data
        inject_agent_infos(env.clone(), agent_infos).unwrap();

        // - Check the same data is now in the store
        let mut agents = all_agent_infos(env.clone().into()).unwrap();

        agents.sort();

        assert_eq!(expect, agents);
    }
}
