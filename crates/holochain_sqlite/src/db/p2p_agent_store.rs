//! p2p_agent_store sql logic

use crate::prelude::*;
use crate::sql::*;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::dht_arc::DhtArcSet;
use kitsune_p2p::KitsuneAgent;
use rusqlite::*;
use std::sync::Arc;

/// Extension trait to treat connection instances
/// as p2p store accessors.
pub trait AsP2pAgentStoreConExt {
    /// Get an AgentInfoSigned record from the p2p_store
    fn p2p_get_agent(&mut self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>>;

    /// List all AgentInfoSigned records within a space in the p2p_agent_store
    fn p2p_list_agents(&mut self) -> DatabaseResult<Vec<AgentInfoSigned>>;

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
}

/// Extension trait to treat transaction instances
/// as p2p store accessors.
pub trait AsP2pStateTxExt {
    /// Get an AgentInfoSigned record from the p2p_store
    fn p2p_get_agent(&self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>>;

    /// List all AgentInfoSigned records within a space in the p2p_agent_store
    fn p2p_list_agents(&self) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Query agent list for gossip
    fn p2p_gossip_query_agents(
        &self,
        since_ms: u64,
        until_ms: u64,
        arcset: DhtArcSet,
    ) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Query agents sorted by nearness to basis loc
    fn p2p_query_near_basis(&self, basis: u32, limit: u32) -> DatabaseResult<Vec<AgentInfoSigned>>;
}

impl AsP2pAgentStoreConExt for crate::db::PConn {
    fn p2p_get_agent(&mut self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>> {
        self.with_reader(move |reader| reader.p2p_get_agent(agent))
    }

    fn p2p_list_agents(&mut self) -> DatabaseResult<Vec<AgentInfoSigned>> {
        self.with_reader(move |reader| reader.p2p_list_agents())
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
}

/// Put an AgentInfoSigned record into the p2p_store
pub async fn p2p_put(
    db: &DbWrite<DbKindP2pAgentStore>,
    signed: &AgentInfoSigned,
) -> DatabaseResult<()> {
    let record = P2pRecord::from_signed(signed)?;
    db.async_commit(move |txn| tx_p2p_put(txn, record)).await
}

/// Put an iterator of AgentInfoSigned records into the p2p_store
pub async fn p2p_put_all(
    db: &DbWrite<DbKindP2pAgentStore>,
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
pub async fn p2p_prune(db: &DbWrite<DbKindP2pAgentStore>) -> DatabaseResult<()> {
    db.async_commit(move |txn| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        txn.execute(sql_p2p_agent_store::PRUNE, named_params! { ":now": now })?;
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
            let interval = info.storage_arc.interval();
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
}

/// Owned data dealing with a full p2p_agent_store record.
#[derive(Debug)]
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

        let storage_center_loc = arc.center_loc().into();

        let is_active = !signed.url_list.is_empty();

        let (storage_start_loc, storage_end_loc) = arc.primitive_range_detached();

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

#[cfg(test)]
mod p2p_test;
