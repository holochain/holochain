//! p2p_agent_store sql logic

use crate::prelude::*;
use crate::sql::*;
use holochain_zome_types::many_bytes_string;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::dht_arc::DhtArcRange;
use kitsune_p2p::dht_arc::DhtArcSet;
use kitsune_p2p::KitsuneAgent;
use rusqlite::*;
use std::sync::Arc;

/// Extension trait to treat connection instances
/// as p2p store accessors.
pub trait AsP2pAgentStoreConExt {
    /// Get an AgentInfoSigned record from the p2p_store
    fn p2p_get_agent(&mut self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>>;

    /// Remove an agent from the p2p store
    fn p2p_remove_agent(&mut self, agent: &KitsuneAgent) -> DatabaseResult<bool>;

    /// List all AgentInfoSigned records within a space in the p2p_agent_store
    fn p2p_list_agents(&mut self) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Count agent records within a space in the p2p_agent_store
    fn p2p_count_agents(&mut self) -> DatabaseResult<u32>;

    /// Query agent list for gossip
    fn p2p_gossip_query_agents(
        &mut self,
        since_ms: u64,
        until_ms: u64,
        arcset: DhtArcSet,
    ) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Query agents sorted by nearness to basis loc
    fn p2p_query_near_basis(
        &mut self,
        basis: u32,
        limit: u32,
    ) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Extrapolate coverage from agents within our own storage arc
    fn p2p_extrapolated_coverage(&mut self, dht_arc_set: DhtArcSet) -> DatabaseResult<Vec<f64>>;
}

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

impl AsP2pAgentStoreConExt for crate::db::PConnGuard {
    fn p2p_get_agent(&mut self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>> {
        self.with_reader(move |reader| reader.p2p_get_agent(agent))
    }

    fn p2p_remove_agent(&mut self, agent: &KitsuneAgent) -> DatabaseResult<bool> {
        self.with_reader(move |reader| reader.p2p_remove_agent(agent))
    }

    fn p2p_list_agents(&mut self) -> DatabaseResult<Vec<AgentInfoSigned>> {
        self.with_reader(move |reader| reader.p2p_list_agents())
    }

    fn p2p_count_agents(&mut self) -> DatabaseResult<u32> {
        self.with_reader(move |reader| reader.p2p_count_agents())
    }

    fn p2p_gossip_query_agents(
        &mut self,
        since_ms: u64,
        until_ms: u64,
        arcset: DhtArcSet,
    ) -> DatabaseResult<Vec<AgentInfoSigned>> {
        self.with_reader(move |reader| reader.p2p_gossip_query_agents(since_ms, until_ms, arcset))
    }

    fn p2p_query_near_basis(
        &mut self,
        basis: u32,
        limit: u32,
    ) -> DatabaseResult<Vec<AgentInfoSigned>> {
        self.with_reader(move |reader| reader.p2p_query_near_basis(basis, limit))
    }

    fn p2p_extrapolated_coverage(&mut self, dht_arc_set: DhtArcSet) -> DatabaseResult<Vec<f64>> {
        self.with_reader(move |reader| reader.p2p_extrapolated_coverage(dht_arc_set))
    }
}

/// Put an AgentInfoSigned record into the p2p_store
pub async fn p2p_put(
    db: &DbWrite<DbKindP2pAgents>,
    signed: &AgentInfoSigned,
) -> DatabaseResult<()> {
    let record = P2pRecord::from_signed(signed)?;
    db.async_commit(move |txn| tx_p2p_put(txn, record)).await
}

/// Put an iterator of AgentInfoSigned records into the p2p_store
pub async fn p2p_put_all(
    db: &DbWrite<DbKindP2pAgents>,
    signed: impl Iterator<Item = &AgentInfoSigned>,
) -> DatabaseResult<()> {
    let mut records = Vec::new();
    for s in signed {
        records.push(P2pRecord::from_signed(s)?);
    }
    db.async_commit(move |txn| {
        for record in records {
            tx_p2p_put(txn, record)?;
        }
        Ok(())
    })
    .await
}

/// Insert a p2p record from within a write transaction.
pub fn p2p_put_single(txn: &mut Transaction<'_>, signed: &AgentInfoSigned) -> DatabaseResult<()> {
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
    db.async_commit(move |txn| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

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
        let mut stmt = self
            .prepare(sql_p2p_agent_store::SELECT)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        Ok(stmt
            .query_row(named_params! { ":agent": &agent.0 }, |r| {
                let r = r.get_ref(0)?;
                let r = r.as_blob()?;
                let signed = AgentInfoSigned::decode(r)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
                Ok(signed)
            })
            .optional()?)
    }

    fn p2p_remove_agent(&self, agent: &KitsuneAgent) -> DatabaseResult<bool> {
        let mut stmt = self
            .prepare(sql_p2p_agent_store::DELETE)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        Ok(stmt.execute(named_params! { ":agent": &agent.0 })? > 0)
    }

    fn p2p_list_agents(&self) -> DatabaseResult<Vec<AgentInfoSigned>> {
        let mut stmt = self
            .prepare(sql_p2p_agent_store::SELECT_ALL)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let mut out = Vec::new();
        for r in stmt.query_map([], |r| {
            let r = r.get_ref(0)?;
            let r = r.as_blob()?;
            let signed = AgentInfoSigned::decode(r)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
            Ok(signed)
        })? {
            out.push(r?);
        }
        Ok(out)
    }

    fn p2p_count_agents(&self) -> DatabaseResult<u32> {
        let count = self.query_row_and_then(sql_p2p_agent_store::COUNT, [], |row| row.get(0))?;
        Ok(count)
    }

    fn p2p_gossip_query_agents(
        &self,
        since_ms: u64,
        until_ms: u64,
        arcset: DhtArcSet,
    ) -> DatabaseResult<Vec<AgentInfoSigned>> {
        let mut stmt = self
            .prepare(sql_p2p_agent_store::GOSSIP_QUERY)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let mut out = Vec::new();
        for r in stmt.query_map(
            named_params! {
                // TODO: just take i64 so we don't need to clamp (probably
                //       should just do more Timestamp refactor)
                ":since_ms": clamp64(since_ms),
                ":until_ms": clamp64(until_ms),
                // we filter by arc in memory, not in the db query
                ":storage_start_loc": Some(u32::MIN),
                ":storage_end_loc": Some(u32::MAX),
            },
            |r| {
                let r = r.get_ref(0)?;
                let r = r.as_blob()?;
                let signed = AgentInfoSigned::decode(r)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

                Ok(signed)
            },
        )? {
            let info = r?;
            let interval = DhtArcRange::from(info.storage_arc);
            if arcset.overlap(&interval.into()) {
                out.push(info);
            }
        }
        Ok(out)
    }

    fn p2p_query_near_basis(&self, basis: u32, limit: u32) -> DatabaseResult<Vec<AgentInfoSigned>> {
        let mut stmt = self
            .prepare(sql_p2p_agent_store::QUERY_NEAR_BASIS)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        let mut out = Vec::new();
        for r in stmt.query_map(named_params! { ":basis": basis, ":limit": limit }, |r| {
            let r = r.get_ref(0)?;
            let r = r.as_blob()?;
            let signed = AgentInfoSigned::decode(r)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

            Ok(signed)
        })? {
            out.push(r?);
        }
        Ok(out)
    }

    fn p2p_extrapolated_coverage(&self, dht_arc_set: DhtArcSet) -> DatabaseResult<Vec<f64>> {
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

        let is_active = !signed.url_list.is_empty();

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

#[cfg(test)]
mod p2p_test;
