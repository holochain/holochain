//! p2p_agent_store sql logic

use crate::prelude::*;
use crate::sql::*;
use holochain_util::hex::many_bytes_string;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::dht_arc::DhtArcRange;
use kitsune_p2p::dht_arc::DhtArcSet;
use kitsune_p2p::KitsuneAgent;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::*;
use std::collections::{hash_map, HashMap};
use std::sync::Arc;

#[cfg(test)]
mod p2p_test;

/// Extension trait to treat transaction instances
/// as p2p store accessors.
pub trait AsP2pStateTxExt {
    /// Get an AgentInfoSigned record from the p2p_store
    fn p2p_get_agent(&self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>>;

    /// Remove an agent from the p2p store
    fn p2p_remove_agent(&self, agent: &KitsuneAgent) -> DatabaseResult<bool>;

    /// List all AgentInfoSigned records within a space in the p2p_agent_store
    fn p2p_list_agents(&self) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Count agent records within a space in the p2p_agent_store
    fn p2p_count_agents(&self) -> DatabaseResult<u32>;

    /// Query agent list for gossip
    fn p2p_gossip_query_agents(
        &self,
        since_ms: u64,
        until_ms: u64,
        arcset: DhtArcSet,
    ) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Query agents sorted by nearness to basis loc
    fn p2p_query_near_basis(&self, basis: u32, limit: u32) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Extrapolate coverage from agents within our own storage arc
    fn p2p_extrapolated_coverage(&self, dht_arc_set: DhtArcSet) -> DatabaseResult<Vec<f64>>;
}

struct AgentStore(Mutex<HashMap<Arc<KitsuneAgent>, AgentInfoSigned>>);

impl AgentStore {
    fn new(con: &Connection) -> DatabaseResult<Self> {
        let mut stmt = con
            .prepare(sql_p2p_agent_store::SELECT_ALL)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let mut map = HashMap::new();
        for r in stmt.query_map([], |r| {
            let r = r.get_ref(0)?;
            let r = r.as_blob()?;
            let signed = AgentInfoSigned::decode(r)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

            Ok(signed)
        })? {
            let r = r?;
            map.insert(r.agent.clone(), r);
        }
        Ok(Self(Mutex::new(map)))
    }

    fn prune(&self, now: u64, local_agents: &[Arc<KitsuneAgent>]) -> DatabaseResult<()> {
        let mut lock = self.0.lock();

        lock.retain(|_, v| {
            for l in local_agents {
                if &v.agent == l {
                    return true;
                }
            }
            v.expires_at_ms > now
        });

        Ok(())
    }

    fn put(&self, agent_info: AgentInfoSigned) -> DatabaseResult<()> {
        let mut lock = self.0.lock();

        if let Some(a) = lock.get(&agent_info.agent) {
            if a.signed_at_ms >= agent_info.signed_at_ms {
                return Ok(());
            }
        }

        lock.insert(agent_info.agent.clone(), agent_info);

        Ok(())
    }

    fn remove(&self, agent: &KitsuneAgent) -> DatabaseResult<()> {
        let _ = self.0.lock().remove(agent);
        Ok(())
    }

    fn get(&self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>> {
        Ok(self.0.lock().get(agent).cloned())
    }

    fn get_all(&self) -> DatabaseResult<Vec<AgentInfoSigned>> {
        Ok(self
            .0
            .lock()
            .values()
            .filter_map(|info| {
                if !info.is_active() {
                    return None;
                }

                Some(info.clone())
            })
            .collect())
    }

    fn count(&self) -> DatabaseResult<u32> {
        Ok(self.0.lock().len() as u32)
    }

    fn query_agents(
        &self,
        since_ms: u64,
        until_ms: u64,
        arcset: DhtArcSet,
    ) -> DatabaseResult<Vec<AgentInfoSigned>> {
        Ok(self
            .0
            .lock()
            .values()
            .filter_map(|info| {
                if !info.is_active() {
                    return None;
                }

                if info.signed_at_ms < since_ms {
                    return None;
                }

                if info.signed_at_ms > until_ms {
                    return None;
                }

                let interval = DhtArcRange::from(info.storage_arc);
                if !arcset.overlap(&interval.into()) {
                    return None;
                }

                Some(info.clone())
            })
            .collect())
    }

    fn query_near_basis(&self, basis: u32, limit: u32) -> DatabaseResult<Vec<AgentInfoSigned>> {
        let lock = self.0.lock();

        let mut out: Vec<(u32, &AgentInfoSigned)> = lock
            .values()
            .filter_map(|v| {
                if v.is_active() {
                    Some((v.storage_arc.dist(basis), v))
                } else {
                    None
                }
            })
            .collect();

        if out.len() > 1 {
            out.sort_by(|a, b| a.0.cmp(&b.0));
        }

        Ok(out
            .into_iter()
            .take(limit as usize)
            .map(|(_, v)| v.clone())
            .collect())
    }
}

struct AgentStoreByPath {
    map: Mutex<HashMap<String, Arc<AgentStore>>>,
}

impl AgentStoreByPath {
    fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, con: &Connection) -> DatabaseResult<Arc<AgentStore>> {
        match self.map.lock().entry(con.path().unwrap().to_string()) {
            hash_map::Entry::Occupied(e) => Ok(e.get().clone()),
            hash_map::Entry::Vacant(e) => {
                let agent_store = Arc::new(AgentStore::new(con)?);
                e.insert(agent_store.clone());
                Ok(agent_store)
            }
        }
    }
}

static CACHE: Lazy<AgentStoreByPath> = Lazy::new(AgentStoreByPath::new);

fn cache_get(con: &Connection) -> DatabaseResult<Arc<AgentStore>> {
    CACHE.get(con)
}

/// Put an AgentInfoSigned record into the p2p_store
pub async fn p2p_put(
    db: &DbWrite<DbKindP2pAgents>,
    signed: &AgentInfoSigned,
) -> DatabaseResult<()> {
    let record = P2pRecord::from_signed(signed)?;
    db.write_async(move |txn| tx_p2p_put(txn, record)).await
}

/// Put an iterator of AgentInfoSigned records into the p2p_store
pub async fn p2p_put_all(
    db: &DbWrite<DbKindP2pAgents>,
    signed: impl Iterator<Item = &AgentInfoSigned>,
) -> DatabaseResult<()> {
    let mut records = Vec::new();
    let mut ns = Vec::new();
    for s in signed {
        ns.push(s.clone());
        records.push(P2pRecord::from_signed(s)?);
    }
    db.write_async(move |txn| {
        for s in ns {
            cache_get(&*txn)?.put(s)?;
        }

        for record in records {
            tx_p2p_put(txn, record)?;
        }
        Ok(())
    })
    .await
}

/// Insert a p2p record from within a write transaction.
pub fn p2p_put_single(txn: &mut Transaction<'_>, signed: &AgentInfoSigned) -> DatabaseResult<()> {
    cache_get(&*txn)?.put(signed.clone())?;
    let record = P2pRecord::from_signed(signed)?;
    tx_p2p_put(txn, record)
}

fn tx_p2p_put(txn: &mut Transaction, record: P2pRecord) -> DatabaseResult<()> {
    txn.execute(
        sql_p2p_agent_store::INSERT,
        named_params! {
            ":agent": &record.agent.0,

            ":encoded": &record.encoded,

            ":signed_at_ms": &record.signed_at_ms,
            ":expires_at_ms": &record.expires_at_ms,
            ":storage_center_loc": &record.storage_center_loc,

            ":is_active": &record.is_active,

            ":storage_start_loc": &record.storage_start_loc,
            ":storage_end_loc": &record.storage_end_loc,
        },
    )?;
    Ok(())
}

/// Prune all expired AgentInfoSigned records from the p2p_store
pub async fn p2p_prune(
    db: &DbWrite<DbKindP2pAgents>,
    local_agents: Vec<Arc<KitsuneAgent>>,
) -> DatabaseResult<()> {
    let mut agent_list = Vec::with_capacity(local_agents.len() * 36);
    for agent in local_agents.iter() {
        agent_list.extend_from_slice(agent.as_ref());
    }
    if agent_list.is_empty() {
        // this is a hack around an apparent bug in sqlite
        // where the delete doesn't run if the subquery returns no rows
        agent_list.extend_from_slice(&[0; 36]);
    }
    db.write_async(move |txn| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        cache_get(&*txn)?.prune(now, &local_agents)?;

        txn.execute(
            sql_p2p_agent_store::PRUNE,
            named_params! {
                ":now": now,
                ":agent_list": agent_list,
            },
        )?;
        DatabaseResult::Ok(())
    })
    .await?;

    Ok(())
}

impl AsP2pStateTxExt for Transaction<'_> {
    fn p2p_get_agent(&self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>> {
        cache_get(self)?.get(agent)
    }

    fn p2p_remove_agent(&self, agent: &KitsuneAgent) -> DatabaseResult<bool> {
        cache_get(self)?.remove(agent)?;

        let mut stmt = self
            .prepare(sql_p2p_agent_store::DELETE)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        Ok(stmt.execute(named_params! { ":agent": &agent.0 })? > 0)
    }

    fn p2p_list_agents(&self) -> DatabaseResult<Vec<AgentInfoSigned>> {
        cache_get(self)?.get_all()
    }

    fn p2p_count_agents(&self) -> DatabaseResult<u32> {
        cache_get(self)?.count()
    }

    fn p2p_gossip_query_agents(
        &self,
        since_ms: u64,
        until_ms: u64,
        arcset: DhtArcSet,
    ) -> DatabaseResult<Vec<AgentInfoSigned>> {
        cache_get(self)?.query_agents(since_ms, until_ms, arcset)
    }

    fn p2p_query_near_basis(&self, basis: u32, limit: u32) -> DatabaseResult<Vec<AgentInfoSigned>> {
        cache_get(self)?.query_near_basis(basis, limit)
    }

    fn p2p_extrapolated_coverage(&self, dht_arc_set: DhtArcSet) -> DatabaseResult<Vec<f64>> {
        // TODO - rewrite this to use the "cache_get" memory cached info
        //        it will run a lot faster than the database query

        let mut stmt = self
            .prepare(sql_p2p_agent_store::EXTRAPOLATED_COVERAGE)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        let mut out = Vec::new();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for interval in dht_arc_set.intervals() {
            match interval {
                DhtArcRange::Full => {
                    out.push(stmt.query_row(
                        named_params! {
                            ":now": now,
                            ":start_loc": 0,
                            ":end_loc": u32::MAX,
                        },
                        |r| r.get(0),
                    )?);
                }
                DhtArcRange::Bounded(start, end) => {
                    out.push(stmt.query_row(
                        named_params! {
                            ":now": now,
                            ":start_loc": (*start).0,
                            ":end_loc": (*end).0,
                        },
                        |r| r.get(0),
                    )?);
                }
                _ => (),
            }
        }

        Ok(out)
    }
}

/// Owned data dealing with a full p2p_agent_store record.
struct P2pRecord {
    agent: Arc<KitsuneAgent>,

    // encoded binary
    encoded: Box<[u8]>,

    // additional queryable fields
    signed_at_ms: i64,
    expires_at_ms: i64,
    storage_center_loc: u32,

    // is this record active?
    is_active: bool,

    // generated fields
    storage_start_loc: Option<u32>,
    storage_end_loc: Option<u32>,
}

/// Clamp a u64 to the range of a i64.
pub fn clamp64(u: u64) -> i64 {
    if u > i64::MAX as u64 {
        i64::MAX
    } else {
        u as i64
    }
}

impl P2pRecord {
    pub fn from_signed(signed: &AgentInfoSigned) -> DatabaseResult<Self> {
        let agent = signed.agent.clone();

        let encoded = signed.encode().map_err(|e| anyhow::anyhow!(e))?;

        let signed_at_ms = signed.signed_at_ms;
        let expires_at_ms = signed.expires_at_ms;
        let arc = signed.storage_arc;

        let storage_center_loc = arc.start_loc().into();

        let is_active = signed.is_active();

        let (storage_start_loc, storage_end_loc) = arc.to_primitive_bounds_detached();

        Ok(Self {
            agent,

            encoded,

            signed_at_ms: clamp64(signed_at_ms),
            expires_at_ms: clamp64(expires_at_ms),
            storage_center_loc,

            is_active,

            storage_start_loc,
            storage_end_loc,
        })
    }
}

impl std::fmt::Debug for P2pRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P2pRecord")
            .field("agent", &self.agent)
            .field("encoded", &many_bytes_string(&self.encoded))
            .field("signed_at_ms", &self.signed_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("storage_center_loc", &self.storage_center_loc)
            .field("is_active", &self.is_active)
            .field("storage_start_loc", &self.storage_start_loc)
            .field("storage_end_loc", &self.storage_end_loc)
            .finish()
    }
}
